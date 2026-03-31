use crate::prelude::*;

use std::{collections::HashMap, ops::Bound, path::Path};

use gio::Application;
use gio::Settings;
use libsignal_service::protocol::DeviceId;
use libsignal_service::{
    Profile,
    content::ContentBody,
    proto::{AttachmentPointer, DataMessage, GroupContextV2},
    protocol::ServiceId,
    sender::{AttachmentSpec, AttachmentUploadError},
    websocket::account::DeviceInfo,
};
#[cfg(target_os = "linux")]
use oo7::Keyring;
use presage::model::groups::Group;
use presage::store::{ContentsStore, StateStore, Thread};
use rand::distr::SampleString;
use url::Url;

use super::{Channel, Contact, Message, manager_thread::ManagerThread};
use crate::backend::message::{DisplayMessage, DisplayMessageExt};
use crate::{ApplicationError, gspawn, tspawn};

const MESSAGE_BOUND: usize = 100;
const MESSAGES_INITIAL_LOAD: usize = 1;
const SCHEMA_ATTRIBUTE: &str = "xdg:schema";
const ATTRIBUTE_PASSWORD: (&str, &str) = ("type", "password");
const SECRET_LENGTH: usize = 64;

gtk::glib::wrapper! {
    /// The manager is the core of the logic of Flare.
    ///
    /// It is mostly a wrapper around [ManagerThread] (which is itself a wrapper around [presage::Manager]).
    /// It also has other functions, which are in use or sending notifications.
    pub struct Manager(ObjectSubclass<imp::Manager>);
}

type StoreType = presage_store_sqlite::SqliteStore;
type PresageError = presage::Error<presage_store_sqlite::SqliteStoreError>;

/// Query the encryption password from the keyring, storing one if none exists.
#[cfg(target_os = "linux")]
async fn encryption_password() -> Result<String, ApplicationError> {
    let keyring = Keyring::new().await?;
    keyring.unlock().await?;
    let attributes = HashMap::from([
        (SCHEMA_ATTRIBUTE, crate::config::BASE_ID),
        ATTRIBUTE_PASSWORD,
    ]);

    log::trace!("Looking up password from libsecret");
    let items = keyring.search_items(&attributes).await?;
    let item = items.first();

    if let Some(item) = item {
        log::trace!("Password found");
        if item.is_locked().await? {
            log::trace!("Item is locked. Unlocking.");
            item.unlock().await?;
        }
        let secret_bytes = item.secret().await?;
        // Should normally not be lossy, but just in case
        let secret = String::from_utf8_lossy(&secret_bytes).into_owned();
        Ok(secret)
    } else {
        log::trace!("Password not found, creating password");
        let distribution = rand::distr::StandardUniform {};
        let secret = distribution.sample_string(&mut rand::rng(), SECRET_LENGTH);
        let secret_bytes = secret.as_bytes();
        log::trace!("Storing password");
        keyring
            .create_item(
                "Flare: Encryption password",
                &attributes,
                secret_bytes,
                true,
            )
            .await?;
        Ok(secret)
    }
}

/// Query the encryption password from the macOS Keychain, storing one if none exists.
#[cfg(target_os = "macos")]
async fn encryption_password() -> Result<String, ApplicationError> {
    use security_framework::passwords::{get_generic_password, set_generic_password};

    log::trace!("Looking up password from macOS Keychain");
    match get_generic_password("Flare", crate::config::BASE_ID) {
        Ok(bytes) => {
            log::trace!("Password found");
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        }
        Err(e) if e.code() == -25300 /* errSecItemNotFound */ => {
            log::trace!("Password not found, creating password");
            let distribution = rand::distr::StandardUniform {};
            let secret = distribution.sample_string(&mut rand::rng(), SECRET_LENGTH);
            log::trace!("Storing password in Keychain");
            set_generic_password("Flare", crate::config::BASE_ID, secret.as_bytes())
                .map_err(ApplicationError::Keychain)?;
            Ok(secret)
        }
        Err(e) => {
            log::error!("Keychain access failed (code {}): {}", e.code(), e);
            Err(ApplicationError::Keychain(e))
        }
    }
}

/// Query the encryption password from the Windows Credential Manager, storing one if none exists.
#[cfg(target_os = "windows")]
async fn encryption_password() -> Result<String, ApplicationError> {
    use windows::{
        core::*,
        Win32::{
            Foundation::*,
            Security::Credentials::*,
        },
    };

    let target_name = "Flare: Encryption password";
    let mut cred: PCREDENTIALW = std::ptr::null_mut();

    unsafe {
        if CredReadW(
            PCWSTR::from_raw(target_name.encode_utf16().chain(Some(0)).collect::<Vec<_>>().as_ptr()),
            CRED_TYPE_GENERIC,
            0,
            &mut cred,
        )
        .as_bool()
        {
            let secret = String::from_utf16_lossy(std::slice::from_raw_parts(
                (*cred).CredentialBlob,
                (*cred).CredentialBlobSize as usize / 2,
            ));
            CredFree(cred as *mut _);
            Ok(secret)
        } else {
            let secret = rand::distr::StandardUniform {}
                .sample_string(&mut rand::rng(), SECRET_LENGTH);
            let secret_bytes = secret.as_bytes();

            let credential = CREDENTIALW {
                Flags: 0,
                Type: CRED_TYPE_GENERIC,
                TargetName: PCWSTR::from_raw(
                    target_name
                        .encode_utf16()
                        .chain(Some(0))
                        .collect::<Vec<_>>()
                        .as_ptr(),
                ),
                Comment: PCWSTR::null(),
                LastWritten: FILETIME::default(),
                CredentialBlobSize: secret_bytes.len() as u32,
                CredentialBlob: secret_bytes.as_ptr() as *mut u8,
                Persist: CRED_PERSIST_ENTERPRISE,
                AttributeCount: 0,
                Attributes: std::ptr::null_mut(),
                TargetAlias: PCWSTR::null(),
                UserName: PCWSTR::null(),
            };

            if CredWriteW(&credential, 0).as_bool() {
                Ok(secret)
            } else {
                Err(ApplicationError::Keychain(
                    std::io::Error::last_os_error().into(),
                ))
            }
        }
    }
}

/// Creating the configuration store at the specified path.
async fn config_store<P: AsRef<Path>>(p: &P) -> Result<StoreType, ApplicationError> {
    let path = p.as_ref();
    log::trace!("Initialize config store at {}", path.to_string_lossy());

    if path.exists() && !path.is_dir() {
        log::error!(
            "Store location already exists and is not a directory: {}",
            path.to_string_lossy()
        );
        return Err(ApplicationError::ConfigurationError(
            crate::ConfigurationError::DbPathNoFolder(path.to_owned()),
        ));
    }

    if !path.exists()
        && let Err(e) = std::fs::create_dir_all(path)
    {
        return Err(ApplicationError::ConfigurationError(
            crate::ConfigurationError::CannotCreateDbFolder(path.to_owned(), e),
        ));
    }

    let passphrase = tspawn!(async { encryption_password().await })
        .await
        .expect("Failed tokio join")?;
    let path = path.to_str().expect("Invalid sqlite store path").to_owned() + "db.sqlite";

    Ok(tspawn!(async move {
        presage_store_sqlite::SqliteStore::open_with_passphrase(
            &path,
            Some(&passphrase),
            presage::model::identity::OnNewIdentity::Trust,
        )
        .await
    })
    .await
    .expect("Failed tokio join")?)
}

impl Manager {
    pub fn new(application: Application) -> Manager {
        let s: Self = Object::new::<Self>();
        s.imp().application.borrow_mut().replace(application);
        s
    }

    pub fn settings(&self) -> Settings {
        self.imp().settings.clone()
    }

    pub fn send_notification(&self, id: Option<String>, notification: &gio::Notification) {
        if self.imp().settings.boolean("notifications")
            && let Some(application) = self.application()
        {
            if self.property("finished-setup") {
                log::trace!("Sending a notification");
                application.send_notification(id.as_deref(), notification);
            } else {
                log::trace!("Adding notification to the pending list");
                self.imp()
                    .pending_notifications
                    .borrow_mut()
                    .push((id, notification.clone()));
            }
        }
    }

    pub fn withdraw_notification(&self, id: &str) {
        if self.imp().settings.boolean("notifications")
            && let Some(application) = self.application()
        {
            if self.property("finished-setup") {
                log::trace!("Withdrawing notification");
                application.withdraw_notification(id);
            } else {
                log::trace!("Adding notification to the pending list");
                self.imp()
                    .pending_notifications
                    .borrow_mut()
                    .retain(|(s, _)| Some(id) != s.as_deref());
            }
        }
    }

    pub fn application(&self) -> Option<Application> {
        self.imp().application.borrow().clone()
    }

    pub async fn clear_registration(&self) -> Result<