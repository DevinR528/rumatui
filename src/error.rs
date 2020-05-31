//! Error conditions.

use std::fmt;

use matrix_sdk::{
    api::{error::ErrorKind, Error as RumaApiError},
    Error as RumaClientError, FromHttpResponseError as RumaResponseError, IntoHttpError,
    ServerError,
};
// use ruma_client_api::error::ErrorKind;
// use serde_json::Error as JsonError;
// use url::ParseError;

#[cfg(feature = "encryption")]
use matrix_sdk_crypto::OlmError;

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
    RumaResponse(String),
    RumaRequest(String),
    UrlParseError(String),
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
            RumaClientError::AuthenticationRequired => Error::NeedAuth(AUTH_MSG.to_string()),
            RumaClientError::RumaResponse(http) => match http {
                RumaResponseError::Http(server) => match server {
                    ServerError::Known(RumaApiError { kind, message, .. }) => match kind {
                        ErrorKind::Forbidden => Error::RumaResponse(LOGIN_MSG.to_string()),
                        ErrorKind::UnknownToken => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::MissingToken => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::BadJson => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::NotJson => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::NotFound => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::LimitExceeded => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::Unknown => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::Unrecognized => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::Unauthorized => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::UserInUse => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::InvalidUsername => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::RoomInUse => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::InvalidRoomState => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::ThreepidInUse => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::ThreepidNotFound => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::ThreepidAuthFailed => {
                            Error::RumaResponse(format!("{}", message))
                        }
                        ErrorKind::ThreepidDenied => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::ServerNotTrusted => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::UnsupportedRoomVersion => {
                            Error::RumaResponse(format!("{}", message))
                        }
                        ErrorKind::IncompatibleRoomVersion => {
                            Error::RumaResponse(format!("{}", message))
                        }
                        ErrorKind::BadState => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::GuestAccessForbidden => {
                            Error::RumaResponse(format!("{}", message))
                        }
                        ErrorKind::CaptchaNeeded => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::CaptchaInvalid => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::MissingParam => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::InvalidParam => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::TooLarge => Error::RumaResponse(format!("{}", message)),
                        ErrorKind::Exclusive => Error::RumaResponse(format!("{}", message)),
                        _ => Error::RumaResponse(format!(
                            "This error is not accounted for, ruma has added error type BUG"
                        )),
                    },
                    ServerError::Unknown(err) => Error::UnknownServer(format!("{}", err)),
                },
                RumaResponseError::Deserialization(err) => Error::SerDeError(format!("{}", err)),
                _ => panic!("ruma-client-api errors have changed rumatui BUG"),
            },
            _ => Error::UnknownServer("".to_string()),
        }
    }
}

impl From<IntoHttpError> for Error {
    fn from(error: IntoHttpError) -> Self {
        let text = format!("{}", error);
        Self::RumaRequest(text)
    }
}
