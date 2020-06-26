use async_zmq::{Message, MultipartIter, SinkExt};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use url::Url;

use super::bano;
use super::cosmogony;
use super::error;
use super::ntfs;
use super::osm;

// From https://gist.github.com/anonymous/ee3e4df093c136ced7b394dc7ffb78e1

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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
}

impl FSM {
    pub fn new<S: Into<String>>(
        index_type: S,
        data_source: S,
        region: S,
        topic: String,
        port: u32,
    ) -> Result<Self, error::Error> {
        let zmq_endpoint = format!("tcp://127.0.0.1:{}", port);
        let zmq = async_zmq::publish(&zmq_endpoint)
            .context(error::ZMQSocketError {
                details: format!("Could not publish on endpoint '{}'", zmq_endpoint),
            })?
            .bind()
            .context(error::ZMQError {
                details: String::from("Could not bind socket for publication"),
            })?;
        Ok(FSM {
            state: State::NotAvailable,
            working_dir: PathBuf::from("./work"),
            mimirs_dir: PathBuf::from("/home/matt/lab/rust/kisio/mimirsbrunn"),
            cosmogony_dir: PathBuf::from("/home/matt/lab/rust/kisio/cosmogony"),
            events: VecDeque::new(),
            es: Url::parse("http://localhost:9200").unwrap(),
            index_type: index_type.into(),
            data_source: data_source.into(),
            region: region.into(),
            topic,
            publish: zmq,
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
            State::NotAvailable => {
                // println!("Not Available");
                // println!("Sending Download Event");
            }
            State::DownloadingInProgress { started_at } => {
                // println!(
                //     "Downloading {} / {} / {}",
                //     self.index_type, self.data_source, self.region
                // );
                match self.data_source.as_ref() {
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
                    "osm" => match osm::download_osm_region(self.working_dir.clone(), &self.region)
                    {
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
                }
            }
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
                // println!(
                //     "Processed {} {} in {}s",
                //     self.data_source,
                //     self.region,
                //     duration.as_secs()
                // );
                self.events.push_back(Event::Index(file_path.clone()));
            }
            State::IndexingInProgress {
                file_path,
                started_at,
            } => {
                // println!(
                //     "Indexing {} / {} / {} using {}",
                //     self.index_type,
                //     self.data_source,
                //     self.region,
                //     file_path.display()
                // );
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
                // println!("Indexing Error: {}", details);
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

    pub async fn exec(&mut self) -> Result<(), error::Error> {
        self.events.push_back(Event::Download);
        while let Some(event) = self.events.pop_front() {
            self.next(event).await;
            let i = self.topic.clone();
            let j = serde_json::to_string(&self.state).unwrap();
            let msg = vec![&i, &j];
            let msg: Vec<Message> = msg.into_iter().map(Message::from).collect();
            let res: MultipartIter<_, _> = msg.into();
            self.publish.send(res).await.unwrap();
            if let State::Failure(string) = &self.state {
                println!("{}", string);
                break;
            } else {
                self.run().await;
            }
        }
        self.publish.close().await.context(error::ZMQSendError {
            details: format!("Could not close publishing endpoint"),
        })
    }
}
