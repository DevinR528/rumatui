//! Error conditions.

use std::fmt;

pub use matrix_sdk::{
    Endpoint, Error as RumaClientError, FromHttpResponseError as RumaResponseError, IntoHttpError,
    ServerError,
};
use ruma_client_api::error::ErrorKind;
use serde_json::Error as JsonError;
use url::ParseError;

#[cfg(feature = "encryption")]
use matrix_sdk_crypto::OlmError;

/// Result type for rumatui.
///
/// Holds more information about the specific error in the forum of `tui::Text`.
/// This allows the `Error` to easily be displayed.
pub type Result<T> = std::result::Result<T, Error>;

/// Internal representation of errors.
#[derive(Debug)]
pub enum Error {
    RumaResponse { text: String, kind: ErrorKind },
    RumaRequest { text: String, error: IntoHttpError },
    UrlParseError { text: String, error: ParseError },
    SerDeError(String),
    NeedAuth(String),
    UnknownServer(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "")
    }
}

impl std::error::Error for Error {}

impl From<RumaClientError> for Error {
    fn from(error: RumaClientError) -> Self {
        match error {
            RumaClientError::AuthenticationRequired => Error::NeedAuth("oops".into()),
            RumaClientError::RumaResponse(http) => match http {
                RumaResponseError::Http(server) => match server {
                    ServerError::Known(matrix_sdk::api::Error { kind, message, .. }) => {
                        Error::RumaResponse {
                            text: message,
                            kind,
                        }
                    }
                    ServerError::Unknown(err) => Error::UnknownServer(err.to_string()),
                },
                RumaResponseError::Deserialization(deser) => Error::SerDeError(deser.to_string()),
                _ => panic!("ruma-client-api errors have changed"),
            },
            _ => Error::UnknownServer("".to_string()),
        }
    }
}

impl From<IntoHttpError> for Error {
    fn from(error: IntoHttpError) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest { text, error }
    }
}
