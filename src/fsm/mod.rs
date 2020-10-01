use async_zmq::{Message, MultipartIter, SinkExt};
use serde::{Deserialize, Serialize};
use slog::{info, o, Logger};
use snafu::ResultExt;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use url::Url;

mod bano;
mod cosmogony;
mod download;
mod ntfs;
mod osm;

use crate::error;
use crate::settings::Settings;

// From https://gist.github.com/anonymous/ee3e4df093c136ced7b394dc7ffb78e1

/// Using internally tagged so it's more ... across types.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum State {
    NotAvailable,
    DownloadingInProgress {
        started_at: SystemTime,
    },
    DownloadingError {
        details: String,
    },
    Downloaded {
        file_path: PathBuf,
        duration: Duration,
    },
    ProcessingInProgress {
        file_path: PathBuf,
        started_at: SystemTime,
    },
    ProcessingError {
        details: String,
    },
    Processed {
        file_path: PathBuf,
        duration: Duration,
    },
    IndexingInProgress {
        file_path: PathBuf,
        started_at: SystemTime,
    },
    IndexingError {
        details: String,
    },
    Indexed {
        duration: Duration,
    },
    ValidationInProgress,
    ValidationError {
        details: String,
    },
    Available,
    Failure(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum Event {
    Download,
    DownloadingError(String),
    DownloadingComplete(PathBuf, Duration),
    Process(PathBuf),
    ProcessingError(String),
    ProcessingComplete(PathBuf, Duration),
    Index(PathBuf),
    IndexingError(String),
    IndexingComplete(Duration),
    Validate,
    ValidationError(String),
    ValidationComplete,
    Reset,
}

pub struct FSM {
    id: i32,                 // Id of the index, used to identify the published notifications.
    state: State,            // Current state of the FSM
    working_dir: PathBuf,    // Where all the files will go (download, processed, ...)
    mimirs_dir: PathBuf,     // Where we can find executables XXX2mimir
    cosmogony_dir: PathBuf,  // Where we can find cosmogony
    events: VecDeque<Event>, // A queue of events
    es: Url,                 // How we connect to elasticsearch
    index_type: String,      // eg admin, streets, addresses, ...
    data_source: String,     // eg OSM, BANO, ...
    region: String,          // The region we need to index
    topic: String,           // The topic we need to broadcast.
    publish: async_zmq::publish::Publish<std::vec::IntoIter<Message>, Message>,
    logger: Logger,
}

impl FSM {
    pub fn new<S: Into<String>>(
        index_id: i32,
        index_type: S,
        data_source: S,
        region: S,
        settings: &Settings,
        topic: S,
        logger: Logger,
    ) -> Result<Self, error::Error> {
        let zmq_endpoint = format!("tcp://{}:{}", settings.zmq.host, settings.zmq.port);
        let zmq = async_zmq::publish(&zmq_endpoint)
            .context(error::ZMQSocketError {
                details: format!("Could not publish on endpoint '{}'", &zmq_endpoint),
            })?
            .bind()
            .context(error::ZMQError {
                details: format!(
                    "Could not bind socket for publication on endpoint '{}'",
                    &zmq_endpoint
                ),
            })?;
        let elasticsearch_endpoint = format!(
            "http://{}:{}",
            settings.elasticsearch.host, settings.elasticsearch.port
        );
        let elasticsearch_url = Url::parse(&elasticsearch_endpoint).context(error::URLError {
            details: format!(
                "Could not parse elasticsearch URL '{}'",
                &elasticsearch_endpoint
            ),
        })?;
        let fsm_logger = logger.new(o!("zmq" => zmq_endpoint));
        Ok(FSM {
            id: index_id,
            state: State::NotAvailable,
            working_dir: PathBuf::from(&settings.work.working_dir),
            mimirs_dir: PathBuf::from(&settings.work.mimirsbrunn_dir),
            cosmogony_dir: PathBuf::from(&settings.work.cosmogony_dir),
            events: VecDeque::new(),
            es: elasticsearch_url,
            index_type: index_type.into(),
            data_source: data_source.into(),
            region: region.into(),
            topic: topic.into(),
            publish: zmq,
            logger: fsm_logger,
        })
    }
    async fn next(&mut self, event: Event) {
        match (&self.state, event) {
            (State::NotAvailable, Event::Download) => {
                self.state = State::DownloadingInProgress {
                    started_at: SystemTime::now(),
                };
            }
            (State::DownloadingInProgress { .. }, Event::DownloadingError(ref d)) => {
                self.state = State::DownloadingError {
                    details: String::from(d.as_str()),
                };
            }
            (State::DownloadingInProgress { .. }, Event::DownloadingComplete(ref p, ref d)) => {
                self.state = State::Downloaded {
                    file_path: p.clone(),
                    duration: d.clone(),
                }
            }
            (State::DownloadingError { .. }, Event::Reset) => {
                self.state = State::NotAvailable;
            }
            (State::Downloaded { .. }, Event::Process(ref p)) => {
                self.state = State::ProcessingInProgress {
                    file_path: p.clone(),
                    started_at: SystemTime::now(),
                };
            }
            (State::ProcessingInProgress { .. }, Event::ProcessingError(d)) => {
                self.state = State::ProcessingError { details: d }
            }
            (State::ProcessingError { .. }, Event::Reset) => {
                self.state = State::NotAvailable;
            }
            (State::ProcessingInProgress { .. }, Event::ProcessingComplete(ref p, ref d)) => {
                self.state = State::Processed {
                    file_path: p.clone(),
                    duration: d.clone(),
                };
            }
            (State::Processed { .. }, Event::Index(ref p)) => {
                self.state = State::IndexingInProgress {
                    file_path: p.clone(),
                    started_at: SystemTime::now(),
                };
            }
            (State::Downloaded { .. }, Event::Index(ref p)) => {
                self.state = State::IndexingInProgress {
                    file_path: p.clone(),
                    started_at: SystemTime::now(),
                };
            }
            (State::IndexingInProgress { .. }, Event::IndexingError(d)) => {
                self.state = State::IndexingError { details: d }
            }
            (State::IndexingError { .. }, Event::Reset) => {
                self.state = State::NotAvailable;
            }
            (State::IndexingInProgress { .. }, Event::IndexingComplete(ref d)) => {
                self.state = State::Indexed {
                    duration: d.clone(),
                };
            }
            (State::Indexed { .. }, Event::Validate) => {
                self.state = State::ValidationInProgress;
            }
            (State::ValidationInProgress, Event::ValidationError(d)) => {
                self.state = State::ValidationError { details: d }
            }
            (State::ValidationError { .. }, Event::Reset) => {
                self.state = State::NotAvailable;
            }
            (State::ValidationInProgress, Event::ValidationComplete) => {
                self.state = State::Available;
            }
            (s, e) => {
                self.state = State::Failure(
                    format!("Wrong state, event combination: {:#?} {:#?}", s, e).to_string(),
                )
            }
        }
    }

    pub async fn run(&mut self) {
        match &self.state {
            State::NotAvailable => {}
            State::DownloadingInProgress { started_at } => match self.data_source.as_ref() {
                "cosmogony" => {
                    match osm::download_osm_region(self.working_dir.clone(), &self.region) {
                        Ok(file_path) => {
                            let duration = started_at.elapsed().unwrap();
                            self.events
                                .push_back(Event::DownloadingComplete(file_path, duration));
                        }
                        Err(err) => {
                            self.events.push_back(Event::DownloadingError(format!(
                                "Could not download: {}",
                                err
                            )));
                        }
                    }
                }
                "bano" => {
                    match bano::download_bano_region(self.working_dir.clone(), &self.region) {
                        Ok(file_path) => {
                            let duration = started_at.elapsed().unwrap();
                            self.events
                                .push_back(Event::DownloadingComplete(file_path, duration));
                        }
                        Err(err) => {
                            self.events.push_back(Event::DownloadingError(format!(
                                "Could not download: {}",
                                err
                            )));
                        }
                    }
                }
                "osm" => match osm::download_osm_region(self.working_dir.clone(), &self.region) {
                    Ok(file_path) => {
                        let duration = started_at.elapsed().unwrap();
                        self.events
                            .push_back(Event::DownloadingComplete(file_path, duration));
                    }
                    Err(err) => {
                        self.events.push_back(Event::DownloadingError(format!(
                            "Could not download: {}",
                            err
                        )));
                    }
                },
                "ntfs" => {
                    match ntfs::download_ntfs_region(self.working_dir.clone(), &self.region) {
                        Ok(file_path) => {
                            let duration = started_at.elapsed().unwrap();
                            self.events
                                .push_back(Event::DownloadingComplete(file_path, duration));
                        }
                        Err(err) => {
                            self.events.push_back(Event::DownloadingError(format!(
                                "Could not download: {}",
                                err
                            )));
                        }
                    }
                }
                _ => {
                    self.events.push_back(Event::DownloadingError(format!(
                        "Dont know how to download {}",
                        &self.data_source
                    )));
                }
            },
            State::DownloadingError { details: _ } => {
                // We can't stay in downloading error state, we need to go back to not available
                // to terminate the fsm
                // It might be the place to do some cleanup
                self.events.push_back(Event::Reset);
            }
            State::Downloaded {
                file_path,
                duration: _,
            } => {
                // We're done downloading, now we need an extra processing step for cosmogony
                match self.data_source.as_ref() {
                    "cosmogony" => {
                        self.events.push_back(Event::Process(file_path.clone()));
                    }
                    _ => {
                        self.events.push_back(Event::Index(file_path.clone()));
                    }
                }
            }
            State::ProcessingInProgress {
                file_path,
                started_at,
            } => match self.data_source.as_ref() {
                "cosmogony" => {
                    match cosmogony::generate_cosmogony(
                        self.cosmogony_dir.clone(),
                        self.working_dir.clone(),
                        file_path.clone(),
                        &self.region,
                    ) {
                        Ok(path) => {
                            let duration = started_at.elapsed().unwrap();
                            self.events
                                .push_back(Event::ProcessingComplete(path, duration));
                        }
                        Err(err) => {
                            self.events.push_back(Event::ProcessingError(format!(
                                "Could not process: {}",
                                err
                            )));
                        }
                    }
                }
                _ => {
                    self.events.push_back(Event::ProcessingError(format!(
                        "Dont know how to process {}",
                        &self.data_source
                    )));
                }
            },
            State::ProcessingError { details: _ } => {
                self.events.push_back(Event::Reset);
            }
            State::Processed {
                file_path,
                duration: _,
            } => {
                self.events.push_back(Event::Index(file_path.clone()));
            }
            State::IndexingInProgress {
                file_path,
                started_at,
            } => {
                match self.data_source.as_ref() {
                    "bano" => {
                        match bano::index_bano_region(
                            self.mimirs_dir.clone(),
                            self.es.clone(),
                            file_path.clone(),
                        ) {
                            Ok(()) => {
                                let duration = started_at.elapsed().unwrap();
                                self.events.push_back(Event::IndexingComplete(duration));
                            }
                            Err(err) => {
                                self.events.push_back(Event::IndexingError(format!(
                                    "Could not index BANO: {}",
                                    err
                                )));
                            }
                        }
                    }
                    "osm" => {
                        // We need to analyze the index_type to see how we are going to import
                        // osm: do we need to import admins, streets, ...?
                        // FIXME: Here, for simplicity, we hard code index_poi = false
                        let index = match self.index_type.as_ref() {
                            "admins" => Some((true, false, false)),
                            "streets" => Some((false, true, false)),
                            _ => None,
                        };

                        if index.is_none() {
                            self.events.push_back(Event::IndexingError(format!(
                                "Could not index {} using OSM",
                                self.index_type
                            )));
                        } else {
                            let index = index.unwrap();
                            match osm::index_osm_region(
                                self.mimirs_dir.clone(),
                                self.es.clone(),
                                file_path.clone(),
                                index.0,
                                index.1,
                                index.2,
                                8, // 8 = default city level
                            ) {
                                Ok(()) => {
                                    let duration = started_at.elapsed().unwrap();
                                    self.events.push_back(Event::IndexingComplete(duration));
                                }
                                Err(err) => {
                                    self.events.push_back(Event::IndexingError(format!(
                                        "Could not index OSM: {}",
                                        err
                                    )));
                                }
                            }
                        }
                    }
                    "cosmogony" => {
                        match cosmogony::index_cosmogony_region(
                            self.mimirs_dir.clone(),
                            self.es.clone(),
                            file_path.clone(),
                        ) {
                            Ok(()) => {
                                let duration = started_at.elapsed().unwrap();
                                self.events.push_back(Event::IndexingComplete(duration));
                            }
                            Err(err) => {
                                self.events.push_back(Event::IndexingError(format!(
                                    "Could not index cosmogony: {}",
                                    err
                                )));
                            }
                        }
                    }
                    "ntfs" => {
                        match ntfs::index_ntfs_region(
                            self.mimirs_dir.clone(),
                            self.es.clone(),
                            file_path.clone(),
                        ) {
                            Ok(()) => {
                                let duration = started_at.elapsed().unwrap();
                                self.events.push_back(Event::IndexingComplete(duration));
                            }
                            Err(err) => {
                                self.events.push_back(Event::IndexingError(format!(
                                    "Could not index NTFS: {}",
                                    err
                                )));
                            }
                        }
                    }
                    _ => {
                        self.events.push_back(Event::IndexingError(format!(
                            "Dont know how to index {}",
                            &self.data_source
                        )));
                    }
                }
            }
            State::IndexingError { details: _ } => {
                self.events.push_back(Event::Reset);
            }
            State::Indexed { duration: _ } => {
                self.events.push_back(Event::Validate);
            }
            State::ValidationInProgress => {
                std::thread::sleep(std::time::Duration::from_secs(1));
                self.events.push_back(Event::ValidationComplete);
            }
            State::ValidationError { details: _ } => {
                self.events.push_back(Event::Reset);
            }
            State::Available => {}
            State::Failure(_) => {}
        }
    }
}

pub async fn exec(mut fsm: FSM) -> Result<(), error::Error> {
    fsm.events.push_back(Event::Download);
    while let Some(event) = fsm.events.pop_front() {
        fsm.next(event).await;
        let i = fsm.topic.clone();
        let j = format!("{}", fsm.id);
        let k = serde_json::to_string(&fsm.state).unwrap();
        let msg = vec![&i, &j, &k]; // topic, index id, status
        let msg: Vec<Message> = msg.into_iter().map(Message::from).collect();
        let res: MultipartIter<_, _> = msg.into();
        info!(
            &fsm.logger,
            "FSM publishing new state {} for index {}", k, j
        );
        fsm.publish.send(res).await.unwrap();
        if let State::Failure(string) = &fsm.state {
            println!("{}", string);
            break;
        } else {
            fsm.run().await;
        }
    }
    fsm.publish.close().await.context(error::ZMQSendError {
        details: format!("Could not close publishing endpoint"),
    })
}

// TODO Move the following in a test
// #[tokio::main]
// async fn main() -> Result<(), error::Error> {
//     // Retrieve command line arguments
//     let matches = App::new("Create Elasticsearch Index")
//         .version("0.1")
//         .author("Matthieu Paindavoine")
//         .arg(
//             Arg::with_name("index_type")
//                 .short("i")
//                 .value_name("STRING")
//                 .help("input type (admins, streets, addresses)"),
//         )
//         .arg(
//             Arg::with_name("data_source")
//                 .short("d")
//                 .value_name("STRING")
//                 .help("data source (osm, bano, openaddress)"),
//         )
//         .arg(
//             Arg::with_name("region")
//                 .short("r")
//                 .value_name("STRING")
//                 .help("region"),
//         )
//         .get_matches();
//
//     let index_type = matches
//         .value_of("index_type")
//         .ok_or(error::Error::MiscError {
//             details: String::from("Missing Index Type"),
//         })?;
//     let data_source = matches
//         .value_of("data_source")
//         .ok_or(error::Error::MiscError {
//             details: String::from("Missing Data Source"),
//         })?;
//     let region = matches.value_of("region").ok_or(error::Error::MiscError {
//         details: String::from("Missing Region"),
//     })?;
//
//     // Now construct and initialize the Finite State Machine (FSM)
//     // state is the name of the topic we're asking the publisher to broadcast message,
//     // 5555 is the port
//     let mut driver =
//         driver::Driver::new(index_type, data_source, region, String::from("state"), 5555)?;
//
//     // Ready a subscription connection to receive notifications from the FSM
//     let mut zmq = async_zmq::subscribe("tcp://127.0.0.1:5555")
//         .context(error::ZMQSocketError {
//             details: String::from("Could not subscribe on tcp://127.0.0.1:5555"),
//         })?
//         .connect()
//         .context(error::ZMQError {
//             details: String::from("Could not connect subscribe"),
//         })?;
//     zmq.set_subscribe("state")
//         .context(error::ZMQSubscribeError {
//             details: format!("Could not subscribe to '{}' topic", "state"),
//         })?;
//
//     // Start the FSM
//     let _ = tokio::spawn(async move { driver.drive().await })
//         .await
//         .context(error::TokioJoinError {
//             details: String::from("Could not run FSM to completion"),
//         })?;
//
//     // and listen for notifications
//     while let Some(msg) = zmq.next().await {
//         // Received message is a type of Result<MessageBuf>
//         let msg = msg.context(error::ZMQRecvError {
//             details: String::from("ZMQ Reception Error"),
//         })?;
//
//         let msg = msg
//             .iter()
//             .skip(1) // skip the topic
//             .next()
//             .ok_or(error::Error::MiscError {
//                 details: String::from("Just one item in a multipart message. That is plain wrong!"),
//             })?;
//         println!("Received: {}", msg.as_str().unwrap());
//         let state = serde_json::from_str(msg.as_str().unwrap()).context(error::SerdeJSONError {
//             details: String::from("Could not deserialize state"),
//         })?;
//
//         match state {
//             driver::State::NotAvailable => {
//                 break;
//             }
//             driver::State::Available => {
//                 break;
//             }
//             _ => {}
//         }
//     }
//     Ok(())
// }
