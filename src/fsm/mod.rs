use async_zmq::StreamExt;
use clap::{App, Arg};
use snafu::ResultExt;

mod bano;
mod cosmogony;
mod download;
mod error;
mod fsm;
mod ntfs;
mod osm;

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
