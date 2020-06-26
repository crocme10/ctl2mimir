use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("IOError {}: {}", details, source))]
    #[snafu(visibility(pub))]
    IOError {
        details: String,
        source: std::io::Error,
    },
    #[snafu(display("ReqwestError {}: {}", details, source))]
    #[snafu(visibility(pub))]
    ReqwestError {
        details: String,
        source: reqwest::Error,
    },
    #[snafu(display("Miscellaneous Error {}", details))]
    #[snafu(visibility(pub))]
    MiscError { details: String },

    #[snafu(display("URL Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    URLError {
        details: String,
        source: url::ParseError,
    },

    #[snafu(display("Serde JSON Error {}: {}", details, source))]
    #[snafu(visibility(pub))]
    SerdeJSONError {
        details: String,
        source: serde_json::error::Error,
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
}
