//! Error conditions.

use std::fmt;
use std::io;

use matrix_sdk::{
    api::{error::ErrorKind, Error as RumaApiError},
    Error as MatrixError, FromHttpResponseError as RumaResponseError, IntoHttpError, ServerError,
};
use matrix_sdk_base::Error as MatrixBaseError;
use tokio::sync::mpsc::error::SendError;
// use ruma_client_api::error::ErrorKind;
use serde_json::Error as JsonError;
use url::ParseError;

use crate::client::client_loop::UserRequest;

/// Result type for rumatui.
///
/// Holds more information about the specific error in the forum of `tui::Text`.
/// This allows the `Error` to easily be displayed.
pub type Result<T> = std::result::Result<T, Error>;

const AUTH_MSG: &str = r#"You tried to reach an endpoint that requires authentication.

This is most likely a bug in `rumatui` or one of it's dependencies."#;

const LOGIN_MSG: &str = r#"The user name or password entered did not match any know user.

Make sure you are logging in on the correct server (rumatui defaults to 'http://matrix.org')."#;

/// Internal representation of errors.
#[derive(Debug)]
pub enum Error {
    Encryption(String),
    RumaResponse(String),
    RumaRequest(String),
    Json(String),
    SerdeJson(JsonError),
    Io(String),
    UrlParseError(String),
    SerDeError(String),
    Matrix(String),
    NeedAuth(String),
    Unknown(String),
    Channel(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Encryption(msg) => write!(f, "{}", msg),
            Self::RumaResponse(msg) => write!(
                f,
                "An error occurred with a response from the server.\n{}",
                msg
            ),
            Self::RumaRequest(msg) => write!(
                f,
                "An error occurred with a request to the server.\n{}",
                msg
            ),
            Self::Io(msg) => write!(f, "An IO error occurred.\n{}", msg),
            Self::Json(msg) => write!(f, "An error occurred parsing a JSON object.\n{}", msg),
            // TODO use the methods on serde_json error
            Self::SerdeJson(msg) => write!(f, "An error occurred parsing a JSON object.\n{}", msg),
            Self::UrlParseError(msg) => {
                write!(f, "An error occurred while parsing a url.\n{}", msg)
            }
            Self::SerDeError(msg) => write!(
                f,
                "An error occurred while serializing or deserializing.\n{}",
                msg
            ),
            Self::Matrix(msg) => write!(
                f,
                "An error occurred in the matrix client library.\n{}",
                msg
            ),
            Self::NeedAuth(msg) => write!(f, "Authentication is required.\n{}", msg),
            Self::Unknown(msg) => write!(f, "An error occurred.\n{}", msg),
            Self::Channel(msg) => write!(
                f,
                "The receiving end of a channel shutdown while still receiving messages.\n{}",
                msg
            ),
        }
    }
}

impl std::error::Error for Error {}

/// This is the most important error conversion as most of the user facing errors are here.
impl From<MatrixError> for Error {
    fn from(error: MatrixError) -> Self {
        match error {
            MatrixError::AuthenticationRequired => Error::NeedAuth(AUTH_MSG.to_string()),
            MatrixError::RumaResponse(http) => match http {
                RumaResponseError::Http(server) => match server {
                    // This should be the most common error kind and some should be recoverable.
                    ServerError::Known(RumaApiError { kind, message, .. }) => match kind {
                        ErrorKind::Forbidden => Error::RumaResponse(LOGIN_MSG.to_string()),
                        _ => Error::RumaResponse(format!("{}", message)),
                    },
                    ServerError::Unknown(err) => Error::Unknown(format!("{}", err)),
                },
                RumaResponseError::Deserialization(err) => Error::SerDeError(format!("{}", err)),
                _ => panic!("ruma-client-api errors have changed rumatui BUG"),
            },
            MatrixError::MatrixError(err) => match err {
                MatrixBaseError::StateStore(err) => Error::Matrix(err),
                MatrixBaseError::SerdeJson(err) => Error::SerdeJson(err),
                MatrixBaseError::AuthenticationRequired => Error::NeedAuth(
                    "An unauthenticated request was made that requires authentication".into(),
                ),
                MatrixBaseError::IoError(err) => Error::Io(format!("{}", err)),
                MatrixBaseError::MegolmError(err) => Error::Encryption(format!("{}", err)),
                MatrixBaseError::OlmError(err) => Error::Encryption(format!("{}", err)),
            },
            _ => Error::Unknown("an Error type was added in matrix-sdk (rumatui BUG)".into()),
        }
    }
}

impl From<MatrixBaseError> for Error {
    fn from(err: MatrixBaseError) -> Self {
        match err {
            MatrixBaseError::StateStore(err) => Error::Matrix(err),
            MatrixBaseError::SerdeJson(err) => Error::SerdeJson(err),
            MatrixBaseError::AuthenticationRequired => Error::NeedAuth(
                "An unauthenticated request was made that requires authentication".into(),
            ),
            MatrixBaseError::IoError(err) => Error::Io(format!("{}", err)),
            MatrixBaseError::MegolmError(err) => Error::Encryption(format!("{}", err)),
            MatrixBaseError::OlmError(err) => Error::Encryption(format!("{}", err)),
        }
    }
}

impl From<IntoHttpError> for Error {
    fn from(error: IntoHttpError) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest(text)
    }
}

impl From<SendError<UserRequest>> for Error {
    fn from(error: SendError<UserRequest>) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest(text)
    }
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest(text)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest(text)
    }
}
