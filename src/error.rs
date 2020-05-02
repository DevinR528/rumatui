//! Error conditions.

use std::fmt;

pub use matrix_sdk::{
    ruma_api, Endpoint, Error as RumaClientError, FromHttpResponseError as RumaResponseError,
    IntoHttpError, ServerError,
};
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
    RumaResponse {
        text: String,
        error: RumaResponseError<RumaClientError>,
    },
    RumaRequest {
        text: String,
        error: IntoHttpError,
    },
    SerDeError {
        text: String,
        error: JsonError,
    },
    UrlParseError {
        text: String,
        error: ParseError,
    },
    NeedAuth(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "")
    }
}

impl std::error::Error for Error {}

impl From<RumaResponseError<RumaClientError>> for Error {
    fn from(error: RumaResponseError<RumaClientError>) -> Self {
        match &error {
            RumaResponseError::Deserialization(de_err) => {
                let text = format!("deserialization failed: {}", de_err);
                Self::RumaResponse { text, error }
            }
            RumaResponseError::Http(server) => match server {
                ruma_api::error::ServerError::Known(e) => {
                    let text = format!("an error occurred with the server: {}", e);
                    Self::RumaResponse { text, error }
                }
                ruma_api::error::ServerError::Unknown(e) => {
                    let text = format!("an unknown error occurred with the server: {}", e);
                    Self::RumaResponse { text, error }
                }
            },
            _ => unreachable!("ruma _NonExhaustive_ found"),
        }
    }
}

impl From<IntoHttpError> for Error {
    fn from(error: IntoHttpError) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest { text, error }
    }
}
