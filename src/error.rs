use std::{error::Error, fmt::Display};

use gtk::glib;
use libsignal_service as lss;
use lss::push_service::ServiceError;
use presage as p;

const FAILED_TO_LOOK_UP_ADDRESS: &str = "failed to lookup address information";
const NETWORK_UNREACHABLE: &str = "Network is unreachable";
const TIMED_OUT: &str = "timed out";
const REQWEST_ERROR: &str = "reqwest error";

type PresageError = presage::Error<presage_store_sled::SledStoreError>;

#[derive(Debug)]
pub enum ConfigurationError {
    DbPathNoFolder(std::path::PathBuf),
}

impl Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigurationError::DbPathNoFolder(p) => {
                writeln!(f, "provided path is not a folder {}", p.display())
            }
        }
    }
}

impl std::error::Error for ConfigurationError {}

#[derive(Debug)]
pub enum ApplicationError {
    IOError(std::io::Error),
    NoInternet,
    Glib(glib::Error),
    Libsecret(oo7::Error),
    Db(presage_store_sled::SledStoreError),
    UnauthorizedSignal,
    // MessageSenderError is pretty big, put into `Box` to move it to the heap.
    SendFailed(Box<libsignal_service::sender::MessageSenderError>),
    ReceiveFailed(libsignal_service::push_service::ServiceError),
    // PresageError is pretty big, put into `Box` to move it to the heap.
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

impl From<oo7::Error> for ApplicationError {
    fn from(e: oo7::Error) -> Self {
        ApplicationError::Libsecret(e)
    }
}

impl From<glib::Error> for ApplicationError {
    fn from(e: glib::Error) -> Self {
        ApplicationError::Glib(e)
    }
}

impl From<presage_store_sled::SledStoreError> for ApplicationError {
    fn from(e: presage_store_sled::SledStoreError) -> Self {
        ApplicationError::Db(e)
    }
}

use gettextrs::gettext;

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationError::IOError(_) => writeln!(
                f,
                "{}",
                gettext("I/O Error.")
            ),
            ApplicationError::NoInternet => writeln!(
                f,
                "{}",
                gettext("There does not seem to be a connection to the internet available.")
            ),
            ApplicationError::Glib(_) => writeln!(
                f,
                "{}",
                gettext("Something glib-related failed.")
            ),
            ApplicationError::Libsecret(_) => writeln!(
                f,
                "{}",
                gettext("The communication with Libsecret failed.")
            ),
            ApplicationError::Db(_) => writeln!(
                f,
                "{}",
                gettext("The backend database failed. Please restart the application or delete the database and relink the application.")
            ),
            ApplicationError::UnauthorizedSignal => writeln!(
                f,
                "{}",
                gettext("You do not seem to be authorized with Signal. Please delete the database and relink the application.")
            ),
            ApplicationError::SendFailed(_) => writeln!(
                f,
                "{}",
                gettext("Sending a message failed.")
            ),
            ApplicationError::ReceiveFailed(_) => writeln!(
                f,
                "{}",
                gettext("Receiving a message failed.")
            ),
            ApplicationError::Presage(_) => writeln!(
                f,
                "{}",
                gettext("Something unexpected happened with the signal backend. Please retry later.")
            ),
            ApplicationError::ConfigurationError(_) => writeln!(
                f,
                "{}",
                gettext("The application seems to be misconfigured.")
            ),
            ApplicationError::ManagerThreadPanic => writeln!(
                f,
                "{}",
                gettext("A part of the application crashed.")
            ),
        }
    }
}

impl ApplicationError {
    pub fn more_information(&self) -> String {
        match self {
            ApplicationError::IOError(e) => format!("{:#?}", e),
            ApplicationError::NoInternet => gettext("Please check your internet connection."),
            ApplicationError::Glib(e) => format!("{:#?}", e),
            ApplicationError::Libsecret(e) => format!("{:#?}", e),
            ApplicationError::Db(e) => format!("{:#?}", e),
            ApplicationError::UnauthorizedSignal => {
                gettext("Please delete the database and relink the device.")
            }
            ApplicationError::SendFailed(e) => format!("{:#?}", e),
            ApplicationError::ReceiveFailed(e) => format!("{:#?}", e),
            ApplicationError::Presage(e) => format!("{:#?}", e),
            ApplicationError::ConfigurationError(e) => match e {
                ConfigurationError::DbPathNoFolder(p) => {
                    let s = gettext("The database path at {} is no folder.");
                    s.replace("{}", &p.to_string_lossy())
                }
            },
            ApplicationError::ManagerThreadPanic => {
                gettext("Please restart the application with logging and report this issue.")
            }
        }
    }

    pub fn should_report(&self) -> bool {
        match self {
            ApplicationError::IOError(_) => false,
            ApplicationError::NoInternet => false,
            ApplicationError::Glib(_) => true,
            ApplicationError::Libsecret(_) => false,
            ApplicationError::Db(_) => true,
            ApplicationError::UnauthorizedSignal => false,
            ApplicationError::SendFailed(_) => true,
            ApplicationError::ReceiveFailed(_) => true,
            ApplicationError::Presage(_) => true,
            ApplicationError::ConfigurationError(_) => true,
            ApplicationError::ManagerThreadPanic => true,
        }
    }
}
