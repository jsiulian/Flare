use libsignal_service as lss;
use presage as p;
use gtk::glib;

const FAILED_TO_LOOK_UP_ADDRESS: &str = "failed to lookup address information";

#[derive(Debug, err_derive::Error)]
pub enum ConfigurationError {
    #[error(display = "Provided path is not a folder")]
    DbPathNoFolder(std::path::PathBuf),
}

#[derive(Debug)]
pub enum ApplicationError {
    IOError(std::io::Error),
    NoInternet,
    Libsecret(glib::error::Error),
    Db(sled::Error),
    UnauthorizedSignal,
    SendFailed(libsignal_service::sender::MessageSenderError),
    ReceiveFailed(libsignal_service::receiver::MessageReceiverError),
    Presage(presage::Error),
    ConfigurationError(ConfigurationError),
    ManagerThreadPanic,
}

impl From<p::Error> for ApplicationError {
    fn from(e: p::Error) -> Self {
        match e {
            p::Error::ServiceError(p::prelude::content::ServiceError::Unauthorized) => {
                ApplicationError::UnauthorizedSignal
            }
            p::Error::DbError(e) => ApplicationError::Db(e),
            p::Error::MessageSenderError(lss::sender::MessageSenderError::NetworkFailure {
                recipient: _,
            }) => ApplicationError::NoInternet,
            p::Error::MessageSenderError(lss::sender::MessageSenderError::ServiceError(
                p::prelude::content::ServiceError::SendError { reason: e },
            )) if e.contains(FAILED_TO_LOOK_UP_ADDRESS) => ApplicationError::NoInternet,
            p::Error::MessageReceiverError(lss::receiver::MessageReceiverError::ServiceError(
                p::prelude::content::ServiceError::WsError { reason: e },
            )) if e.contains(FAILED_TO_LOOK_UP_ADDRESS) => ApplicationError::NoInternet,
            p::Error::MessageReceiverError(e) => ApplicationError::ReceiveFailed(e),
            p::Error::MessageSenderError(e) => ApplicationError::SendFailed(e),
            _ => ApplicationError::Presage(e),
        }
    }
}

impl From<std::io::Error> for ApplicationError {
    fn from(e: std::io::Error) -> Self {
        ApplicationError::IOError(e)
    }
}

impl From<glib::error::Error> for ApplicationError {
    fn from(e: glib::error::Error) -> Self {
        ApplicationError::Libsecret(e)
    }
}

impl From<sled::Error> for ApplicationError {
    fn from(e: sled::Error) -> Self {
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
            ApplicationError::Libsecret(_) => false,
            ApplicationError::Db(_) => true,
            ApplicationError::UnauthorizedSignal => false,
            ApplicationError::SendFailed(_) => false,
            ApplicationError::ReceiveFailed(_) => false,
            ApplicationError::Presage(_) => true,
            ApplicationError::ConfigurationError(_) => false,
            ApplicationError::ManagerThreadPanic => true,
        }
    }
}
