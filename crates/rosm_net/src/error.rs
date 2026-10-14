//! Error definitions for OSM API, Overpass QL, and OAuth networking operations.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("HTTP error {status}: {message}")]
    Http { status: u16, message: String },

    #[error("Conflict detected (HTTP 409): {message}")]
    Conflict { message: String },

    #[error("Resource not found (HTTP 404): {primitive}")]
    NotFound { primitive: String },

    #[error("Network connection error: {0}")]
    Network(String),

    #[error("XML parsing error: {0}")]
    XmlParse(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Invalid request or data: {0}")]
    InvalidData(String),

    #[error("URL format error: {0}")]
    Url(#[from] url::ParseError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ureq::Error> for NetError {
    fn from(err: ureq::Error) -> Self {
        match err {
            ureq::Error::Status(code, resp) => {
                let msg = resp
                    .into_string()
                    .unwrap_or_else(|_| "Unknown HTTP error".to_string());
                match code {
                    409 => NetError::Conflict { message: msg },
                    404 => NetError::NotFound { primitive: msg },
                    _ => NetError::Http {
                        status: code,
                        message: msg,
                    },
                }
            }
            ureq::Error::Transport(t) => NetError::Network(t.to_string()),
        }
    }
}
