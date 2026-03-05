use std::path::Path;

use rand::distr::SampleString;

use super::error::{ApplicationError, ConfigurationError};

type StoreType = presage_store_sqlite::SqliteStore;

const SCHEMA_ATTRIBUTE: &str = "xdg:schema";
const ATTRIBUTE_PASSWORD: (&str, &str) = ("type", "password");
const SECRET_LENGTH: usize = 64;
const BASE_ID: &str = "de.schmidhuberj.Flare";

#[cfg(target_os = "linux")]
pub async fn encryption_password() -> Result<String, ApplicationError> {
    use oo7::Keyring;
    use std::collections::HashMap;

    let keyring = Keyring::new().await?;
    keyring.unlock().await?;
    let attributes = HashMap::from([(SCHEMA_ATTRIBUTE, BASE_ID), ATTRIBUTE_PASSWORD]);

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
        let secret = String::from_utf8_lossy(&secret_bytes).into_owned();
        Ok(secret)
    } else {
        log::trace!("Password not found, creating password");
        let distribution = rand::distr::StandardUniform {};
        let secret = distribution.sample_string(&mut rand::rng(), SECRET_LENGTH);
        let secret_bytes = secret.as_bytes();
        log::trace!("Storing password");
        keyring
            .create_item("Flare: Encryption password", &attributes, secret_bytes, true)
            .await?;
        Ok(secret)
    }
}

#[cfg(target_os = "macos")]
pub async fn encryption_password() -> Result<String, ApplicationError> {
    use security_framework::passwords::{get_generic_password, set_generic_password};

    log::trace!("Looking up password from macOS Keychain");
    match get_generic_password("Flare", BASE_ID) {
        Ok(bytes) => {
            log::trace!("Password found");
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        }
        Err(e) if e.code() == -25300 /* errSecItemNotFound */ => {
            log::trace!("Password not found, creating password");
            let distribution = rand::distr::StandardUniform {};
            let secret = distribution.sample_string(&mut rand::rng(), SECRET_LENGTH);
            log::trace!("Storing password in Keychain");
            set_generic_password("Flare", BASE_ID, secret.as_bytes())
                .map_err(ApplicationError::Keychain)?;
            Ok(secret)
        }
        Err(e) => {
            log::error!("Keychain access failed (code {}): {}", e.code(), e);
            Err(ApplicationError::Keychain(e))
        }
    }
}

pub async fn config_store(p: &Path) -> Result<StoreType, ApplicationError> {
    log::trace!("Initialize config store at {}", p.to_string_lossy());

    if p.exists() && !p.is_dir() {
        log::error!(
            "Store location already exists and is not a directory: {}",
            p.to_string_lossy()
        );
        return Err(ApplicationError::ConfigurationError(
            ConfigurationError::DbPathNoFolder(p.to_owned()),
        ));
    }

    if !p.exists() {
        if let Err(e) = std::fs::create_dir_all(p) {
            return Err(ApplicationError::ConfigurationError(
                ConfigurationError::CannotCreateDbFolder(p.to_owned(), e),
            ));
        }
    }

    let passphrase = encryption_password().await?;
    let path = p.to_str().expect("Invalid sqlite store path").to_owned() + "db.sqlite";

    Ok(presage_store_sqlite::SqliteStore::open_with_passphrase(
        &path,
        Some(&passphrase),
        presage::model::identity::OnNewIdentity::Trust,
    )
    .await?)
}
