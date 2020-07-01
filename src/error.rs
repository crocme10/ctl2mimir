use juniper::{graphql_value, FieldError, IntoFieldError};
use snafu::{Backtrace, Snafu};
use std::io;

use crate::db::model::ProvideError;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Misc Error: {}", details))]
    #[snafu(visibility(pub))]
    MiscError { details: String },

    #[snafu(display("Environment Variable Error: {} => {}", details, source))]
    #[snafu(visibility(pub))]
    EnvError {
        details: String,
        source: dotenv::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("IO Error: {}", source))]
    #[snafu(visibility(pub))]
    IOError {
        source: io::Error,
        details: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Reqwest Error: {} {}", details, source))]
    #[snafu(visibility(pub))]
    ReqwestError {
        source: reqwest::Error,
        details: String,
        backtrace: Backtrace,
    },

    #[snafu(display("UrL Error: {} {}", details, source))]
    #[snafu(visibility(pub))]
    URLError {
        source: url::ParseError,
        details: String,
    },

    #[snafu(display("Tokio IO Error: {}", source))]
    #[snafu(visibility(pub))]
    TokioIOError {
        source: tokio::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Tokio Task Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    TokioJoinError {
        details: String,
        source: tokio::task::JoinError,
    },

    #[snafu(display("ZeroMQ Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    ZMQError {
        details: String,
        source: async_zmq::Error,
    },

    #[snafu(display("ZeroMQ Subscribe Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    ZMQSubscribeError {
        details: String,
        source: async_zmq::SubscribeError,
    },

    #[snafu(display("ZeroMQ Socket Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    ZMQSocketError {
        details: String,
        source: async_zmq::SocketError,
    },

    #[snafu(display("ZeroMQ Receive Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    ZMQRecvError {
        details: String,
        source: async_zmq::RecvError,
    },

    #[snafu(display("ZeroMQ Send Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    ZMQSendError {
        details: String,
        source: async_zmq::SendError,
    },
    #[snafu(display("DB Error: {} => {}", details, source))]
    #[snafu(visibility(pub))]
    DBError {
        details: String,
        source: sqlx::error::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Serde Json Error: {} => {}", details, source))]
    #[snafu(visibility(pub))]
    SerdeJSONError {
        details: String,
        source: serde_json::error::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("DB Provider Error: {} => {}", details, source))]
    #[snafu(visibility(pub))]
    DBProvideError {
        details: String,
        source: ProvideError,
        backtrace: Backtrace,
    },

    #[snafu(display("Parse Int Error: {} => {}", details, source))]
    #[snafu(visibility(pub))]
    ParseIntError {
        details: String,
        source: std::num::ParseIntError,
    },
}

impl IntoFieldError for Error {
    fn into_field_error(self) -> FieldError {
        match self {
            err @ Error::MiscError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("User Error", graphql_value!({ "internal_error": errmsg }))
            }
            err @ Error::EnvError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Environment Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }
            err @ Error::IOError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("IO Error", graphql_value!({ "internal_error": errmsg }))
            }
            err @ Error::TokioIOError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Tokio IO Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }
            err @ Error::DBError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Database Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }
            err @ Error::SerdeJSONError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("Serde Error", graphql_value!({ "internal_error": errmsg }))
            }

            err @ Error::DBProvideError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "DB Provide Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ReqwestError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Reqwest Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::URLError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("URL Error", graphql_value!({ "internal_error": errmsg }))
            }

            err @ Error::TokioJoinError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Tokio Join Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ZMQError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("ZMQ Error", graphql_value!({ "internal_error": errmsg }))
            }

            err @ Error::ZMQSubscribeError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "ZMQ Subscribe Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ZMQSocketError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "ZMQ Socket Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ZMQRecvError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "ZMQ Receive Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ZMQSendError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "ZMQ Send Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ParseIntError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Parse Int Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }
        }
    }
}
