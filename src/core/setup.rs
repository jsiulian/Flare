use std::cell::OnceCell;

use futures::{SinkExt, join};
use libsignal_service::configuration::SignalServers;
use libsignal_service::prelude::phonenumber;
use presage::manager::{Registered, RegistrationOptions};
use presage_store_sqlite::SqliteStore as Store;
use url::Url;

type Error = presage::Error<<Store as presage::store::Store>::Error>;

/// Message from the UI to the manager thread on how to do setup.
#[derive(Debug)]
pub enum SetupDecision {
    /// Server, Device Name
    Link(SignalServers, String),
    /// Server, Phone Number, Captcha
    Register(SignalServers, phonenumber::PhoneNumber, String),
}

pub type SetupConfirmation = String;

/// Message from the manager thread to the UI on what steps need to be taken for setting up.
#[derive(Debug)]
pub enum SetupResult {
    /// Setup must make a decision. Either register as primary device or link device.
    Pending(OnceCell<futures::channel::oneshot::Sender<SetupDecision>>),
    /// The manager is pending a SMS confirmation code.
    Confirm(OnceCell<futures::channel::oneshot::Sender<SetupConfirmation>>),
    /// Display QR code to link.
    DisplayLinkQR(Url),
    /// Everything is finished
    Finished,
}

/// Setting up the presage manager if required.
pub async fn setup_manager(
    config_store: Store,
    mut setup_sender: futures::channel::mpsc::Sender<SetupResult>,
) -> Result<presage::Manager<Store, Registered>, Error> {
    if let Ok(manager) = presage::Manager::load_registered(config_store.clone()).await {
        log::debug!("Config store already valid, loading registered account");
        setup_sender
            .send(SetupResult::Finished)
            .await
            .expect("Failed to send setup results");
        Ok(manager)
    } else {
        log::debug!("Config store not valid yet, requesting setup decision");

        let (tx_decision, rx_decision) = futures::channel::oneshot::channel();
        let to_send = OnceCell::new();
        let _ = to_send.set(tx_decision);
        setup_sender
            .send(SetupResult::Pending(to_send))
            .await
            .expect("Failed to send setup results");

        match rx_decision.await.expect("Callback receiving failed") {
            SetupDecision::Link(servers, name) => {
                let (tx_link, rx_link) = futures::channel::oneshot::channel();
                let (_, mut manager) = join!(
                    async {
                        let link = rx_link.await.expect("Failed to receive link callback");
                        setup_sender
                            .send(SetupResult::DisplayLinkQR(link))
                            .await
                            .expect("Failed to send setup results");
                    },
                    presage::Manager::link_secondary_device(
                        config_store.clone(),
                        servers,
                        name,
                        tx_link,
                    )
                );
                if let Ok(ref mut m) = manager {
                    if let Err(e) = m.request_contacts().await {
                        log::error!("Failed to sync contacts after linking: {}", e);
                    }
                    setup_sender
                        .send(SetupResult::Finished)
                        .await
                        .expect("Failed to send setup results");
                }
                manager
            }
            SetupDecision::Register(servers, phonenumber, captcha) => {
                let (tx_confirm, rx_confirm) = futures::channel::oneshot::channel();
                let manager = presage::Manager::register(
                    config_store.clone(),
                    RegistrationOptions {
                        signal_servers: servers,
                        phone_number: phonenumber,
                        use_voice_call: false,
                        captcha: Some(&captcha[..]),
                        force: false,
                    },
                )
                .await?;
                setup_sender
                    .send(SetupResult::Confirm(tx_confirm.into()))
                    .await
                    .expect("Failed to send setup results");
                let confirmation = rx_confirm
                    .await
                    .expect("Failed to receive confirm callback");
                let manager = manager.confirm_verification_code(&confirmation).await;
                setup_sender
                    .send(SetupResult::Finished)
                    .await
                    .expect("Failed to send setup results");
                manager
            }
        }
    }
}
