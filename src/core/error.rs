use std::{error::Error, fmt::Display};

use libsignal_service as lss;
use lss::push_service::ServiceError;
use presage as p;

const FAILED_TO_LOOK_UP_ADDRESS: &str = "failed to lookup address information";
const NETWORK_UNREACHABLE: &str = "Network is unreachable";
const TIMED_OUT: &str = "timed out";
const REQWEST_ERROR: &str = "reqwest error";

type PresageError = presage::Error<presage_store_sqlite::SqliteStoreError>;

#[derive(Debug)]
pub enum ConfigurationError {
    DbPathNoFolder(std::path::PathBuf),
    CannotCreateDbFolder(std::path::PathBuf, std::io::Error),
}

impl Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigurationError::DbPathNoFolder(p) => {
                writeln!(f, "provided path is not a folder {}", p.display())
            }
            ConfigurationError::CannotCreateDbFolder(p, e) => {
                writeln!(
                    f,
                    "cannot create the database folder {}: {}",
                    p.display(),
                    e
                )
            }
        }
    }
}

impl std::error::Error for ConfigurationError {}

#[derive(Debug)]
pub enum ApplicationError {
    IOError(std::io::Error),
    NoInternet,
    #[cfg(target_os = "linux")]
    Libsecret(oo7::Error),
    #[cfg(target_os = "macos")]
    Keychain(security_framework::base::Error),
    Db(presage_store_sqlite::SqliteStoreError),
    UnauthorizedSignal,
    SendFailed(Box<libsignal_service::sender::MessageSenderError>),
    ReceiveFailed(libsignal_service::push_service::ServiceError),
    Presage(Box<PresageError>),
    ConfigurationError(ConfigurationError),
    ManagerThreadPanic,
}

impl From<PresageError> for ApplicationError {
    fn from(e: PresageError) -> Self {
        match e {
            p::Error::ServiceError(ServiceError::Unauthorized) => {
                ApplicationError::UnauthorizedSignal
            }
            p::Error::Store(e) => ApplicationError::Db(e),
            p::Error::ServiceError(ServiceError::WsError(e))
                if e.to_string().contains(FAILED_TO_LOOK_UP_ADDRESS)
                    || e.to_string().contains(NETWORK_UNREACHABLE)
                    || e.to_string().contains(TIMED_OUT)
                    || e.to_string().contains(REQWEST_ERROR)
                    || e.source().is_some_and(|s| {
                        s.to_string().contains(FAILED_TO_LOOK_UP_ADDRESS)
                            || s.to_string().contains(NETWORK_UNREACHABLE)
                            || s.to_string().contains(TIMED_OUT)
                            || s.to_string().contains(REQWEST_ERROR)
                    }) =>
            {
                ApplicationError::NoInternet
            }
            p::Error::MessageSenderError(e) => match *e {
                lss::sender::MessageSenderError::ServiceError(ServiceError::SendError {
                    reason: e,
                }) if e.contains(FAILED_TO_LOOK_UP_ADDRESS) => ApplicationError::NoInternet,
                _ => ApplicationError::SendFailed(e),
            },
            p::Error::ServiceError(ServiceError::SendError { reason: e })
                if e.contains(FAILED_TO_LOOK_UP_ADDRESS) =>
            {
                ApplicationError::NoInternet
            }
            _ => ApplicationError::Presage(Box::new(e)),
        }
    }
}

impl From<std::io::Error> for ApplicationError {
    fn from(e: std::io::Error) -> Self {
        ApplicationError::IOError(e)
    }
}

#[cfg(target_os = "linux")]
impl From<oo7::Error> for ApplicationError {
    fn from(e: oo7::Error) -> Self {
        ApplicationError::Libsecret(e)
    }
}

#[cfg(target_os = "macos")]
impl From<security_framework::base::Error> for ApplicationError {
    fn from(e: security_framework::base::Error) -> Self {
        ApplicationError::Keychain(e)
    }
}

impl From<presage_store_sqlite::SqliteStoreError> for ApplicationError {
    fn from(e: presage_store_sqlite::SqliteStoreError) -> Self {
        ApplicationError::Db(e)
    }
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationError::IOError(_) => write!(f, "I/O Error"),
            ApplicationError::NoInternet => {
                write!(f, "No internet connection available")
            }
            #[cfg(target_os = "linux")]
            ApplicationError::Libsecret(_) => {
                write!(f, "Communication with libsecret failed")
            }
            #[cfg(target_os = "macos")]
            ApplicationError::Keychain(_) => {
                write!(f, "Communication with the macOS Keychain failed")
            }
            ApplicationError::Db(_) => write!(
                f,
                "Backend database failed. Please restart or delete the database and relink."
            ),
            ApplicationError::UnauthorizedSignal => write!(
                f,
                "Not authorized with Signal. Please delete the database and relink."
            ),
            ApplicationError::SendFailed(_) => write!(f, "Sending a message failed"),
            ApplicationError::ReceiveFailed(_) => write!(f, "Receiving a message failed"),
            ApplicationError::Presage(_) => {
                write!(f, "Unexpected error in signal backend. Please retry later.")
            }
            ApplicationError::ConfigurationError(_) => {
                write!(f, "Application is misconfigured")
            }
            ApplicationError::ManagerThreadPanic => {
                write!(f, "A part of the application crashed")
            }
        }
    }
}
