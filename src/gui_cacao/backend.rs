use std::sync::{Arc, Mutex};

use cacao::appkit::App;
use futures::StreamExt;
use presage::manager::Registered;
use presage::store::{ContentsStore, StateStore, Thread};
use presage_store_sqlite::SqliteStore as Store;

use crate::core::channel::{load_channels, ChannelId};
use crate::core::message::{load_messages, resolve_sender_name, CoreMessage, CoreAttachment};
use crate::core::setup::{SetupDecision, SetupResult};
use crate::core::store::config_store;

use super::app::{AppMessage, FlareApp};
use super::linked_devices::DeviceEntry;

pub enum BackendCommand {
    LoadMessages(ChannelId),
    LoadOlderMessages(ChannelId, usize),
    SendMessage(ChannelId, String, Option<crate::core::message::QuoteData>),
    SendAttachment(ChannelId, std::path::PathBuf),
    SendReaction(ChannelId, u64, libsignal_service::protocol::ServiceId, String),
    FetchDevices,
    SyncContacts,
    UnlinkDevice { keep_data: bool },
    ClearAllMessages,
    ClearChannelMessages(ChannelId),
    OpenAttachment(libsignal_service::proto::AttachmentPointer),
    DownloadAttachment(ChannelId, u64, libsignal_service::proto::AttachmentPointer),
    DeleteMessage(ChannelId, u64),
    SubmitCaptcha(String, String),
}

pub struct BackendState {
    pub setup_decision_tx: Mutex<Option<futures::channel::oneshot::Sender<SetupDecision>>>,
    pub confirm_tx: Mutex<Option<futures::channel::oneshot::Sender<String>>>,
    pub command_tx: Mutex<Option<futures::channel::mpsc::UnboundedSender<BackendCommand>>>,
}

impl Default for BackendState {
    fn default() -> Self {
        Self {
            setup_decision_tx: Mutex::new(None),
            confirm_tx: Mutex::new(None),
            command_tx: Mutex::new(None),
        }
    }
}

pub fn start_backend(state: Arc<BackendState>) {
    std::thread::Builder::new()
        .name("FlareBackend".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let data_dir = dirs::data_dir()
                    .expect("Failed to find data directory")
                    .join("flare");

                log::trace!("Opening config store at {:?}", data_dir);
                let store = match config_store(&data_dir).await {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("Failed to open config store: {}", e);
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SetupError(
                            format!("Failed to open database: {}", e),
                        ));
                        return;
                    }
                };

                let store_for_channels = store.clone();

                let (setup_tx, mut setup_rx) =
                    futures::channel::mpsc::channel::<SetupResult>(10);

                let state_clone = state.clone();
                let (manager_result, _) = futures::join!(
                    crate::core::setup::setup_manager(store, setup_tx),
                    async {
                        while let Some(result) = setup_rx.next().await {
                            match result {
                                SetupResult::Pending(cell) => {
                                    if let Some(tx) = cell.into_inner() {
                                        *state_clone.setup_decision_tx.lock().unwrap() = Some(tx);
                                    }
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::SetupPending,
                                    );
                                }
                                SetupResult::DisplayLinkQR(url) => {
                                    let png_bytes = qrcode_generator::to_png_to_vec(
                                        url.to_string(),
                                        qrcode_generator::QrCodeEcc::Low,
                                        200,
                                    )
                                    .expect("Failed to generate QR code");
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::DisplayQR(png_bytes),
                                    );
                                }
                                SetupResult::Confirm(cell) => {
                                    if let Some(tx) = cell.into_inner() {
                                        *state_clone.confirm_tx.lock().unwrap() = Some(tx);
                                    }
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::RequestConfirmation,
                                    );
                                }
                                SetupResult::Finished => {
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::SetupFinished,
                                    );
                                }
                            }
                        }
                    }
                );

                match manager_result {
                    Ok(manager) => {
                        log::trace!("Setup complete, manager ready");

                        let channels = load_channels(&store_for_channels).await;
                        log::trace!("Loaded {} channels", channels.len());
                        App::<FlareApp, AppMessage>::dispatch_main(
                            AppMessage::ChannelsLoaded(channels),
                        );

                        let (cmd_tx, cmd_rx) = futures::channel::mpsc::unbounded();
                        *state.command_tx.lock().unwrap() = Some(cmd_tx);

                        run_message_loop(manager, store_for_channels, cmd_rx).await;
                    }
                    Err(e) => {
                        log::error!("Setup failed: {}", e);
                        // Delete the database file entirely so the next attempt generates fresh
                        // identity/pre-keys. clear_registration() alone is not enough — presage
                        // reuses stale keys which Signal rejects with 409.
                        drop(store_for_channels);
                        let db_path = data_dir.to_str().unwrap_or("").to_owned() + "db.sqlite";
                        if let Err(re) = std::fs::remove_file(&db_path) {
                            log::warn!("Failed to remove stale database: {}", re);
                        } else {
                            log::info!("Removed stale database at {}", db_path);
                        }
                        let msg = if e.to_string().contains("409") {
                            "Linking failed (conflict with Signal server).\n\nOn your primary Signal device go to Settings → Linked Devices, remove any stale Flare entry, then tap Try Again.".to_string()
                        } else {
                            format!("Setup failed: {}", e)
                        };
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SetupError(msg));
                    }
                }
            });
        })
        .expect("Failed to spawn backend thread");
}

async fn run_message_loop(
    mut manager: presage::Manager<Store, Registered>,
    mut store: Store,
    mut commands: futures::channel::mpsc::UnboundedReceiver<BackendCommand>,
) {
    use futures::{FutureExt, select};
    use presage::model::messages::Received;

    loop {
        log::trace!("Starting message receive loop");
        match manager.receive_messages().await {
            Ok(messages) => {
                futures::pin_mut!(messages);

                loop {
                    select! {
                        msg = messages.next().fuse() => {
                            match msg {
                                Some(Received::QueueEmpty) => {
                                    log::trace!("Message queue empty — reloading channels");
                                    let channels = load_channels(&store).await;
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::ChannelsLoaded(channels),
                                    );
                                }
                                Some(Received::Contacts) => {
                                    log::trace!("Received contacts — reloading channels");
                                    let channels = load_channels(&store).await;
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::ChannelsLoaded(channels),
                                    );
                                }
                                Some(Received::Content(content)) => {
                                    if let Some(mut core_msg) = CoreMessage::from_content(&content) {
                                        if core_msg.is_typing {
                                            if let Some(cid) = core_msg.channel_id.clone() {
                                                App::<FlareApp, AppMessage>::dispatch_main(
                                                    AppMessage::TypingStarted(cid),
                                                );
                                            }
                                        } else if core_msg.is_receipt {
                                            use libsignal_service::content::ContentBody;
                                            if let ContentBody::ReceiptMessage(rm) = &content.body {
                                                let ts: Vec<u64> = rm.timestamp.clone();
                                                let is_read = rm.r#type.map_or(false, |t| t >= 1);
                                                if !ts.is_empty() {
                                                    App::<FlareApp, AppMessage>::dispatch_main(
                                                        AppMessage::ReceiptsReceived(ts, is_read),
                                                    );
                                                }
                                            }
                                        } else if !core_msg.is_receipt {
                                            core_msg.sender_name = resolve_sender_name(&store, &core_msg.sender).await;
                                            // Download image inline so the preview shows immediately.
                                            if super::preferences_window::download_images() {
                                                if let Some(ptr) = core_msg.attachments.iter().find(|p| is_image_attachment(p)) {
                                                    core_msg.image_path = cache_image_attachment(&manager, ptr).await;
                                                    if core_msg.image_path.is_some() {
                                                        if let Some(ref b) = core_msg.body.clone() {
                                                            if b.starts_with('\u{1f5bc}') || b.starts_with('\u{1f3ac}') {
                                                                core_msg.body = None;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            if let Some(cid) = core_msg.channel_id.clone() {
                                                App::<FlareApp, AppMessage>::dispatch_main(
                                                    AppMessage::ChannelMessageReceived(
                                                        cid,
                                                        core_msg.body.clone(),
                                                        core_msg.timestamp,
                                                    ),
                                                );
                                            }
                                            App::<FlareApp, AppMessage>::dispatch_main(
                                                AppMessage::NewMessage(core_msg),
                                            );
                                            // Schedule a delayed channel reload so presage has
                                            // time to save the contact profile ("on first sight")
                                            // before we re-resolve names and titles.
                                            let store_clone = store.clone();
                                            tokio::task::spawn_local(async move {
                                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                                let channels = load_channels(&store_clone).await;
                                                App::<FlareApp, AppMessage>::dispatch_main(
                                                    AppMessage::ChannelsLoaded(channels),
                                                );
                                            });
                                        }
                                    }
                                }
                                None => {
                                    log::warn!("Message stream ended, restarting...");
                                    break;
                                }
                            }
                        }
                        cmd = commands.next().fuse() => {
                            match cmd {
                                Some(BackendCommand::LoadMessages(channel_id)) => {
                                    let msgs = load_messages_with_images(&manager, &store, &channel_id, 50, 0).await;
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::MessagesLoaded(channel_id, msgs),
                                    );
                                }
                                Some(BackendCommand::LoadOlderMessages(channel_id, offset)) => {
                                    let msgs = load_messages_with_images(&manager, &store, &channel_id, 50, offset).await;
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::OlderMessagesLoaded(msgs),
                                    );
                                }
                                Some(BackendCommand::SendMessage(channel_id, text, quote)) => {
                                    send_message(&mut manager, &store, &channel_id, &text, quote).await;
                                }
                                Some(BackendCommand::SendAttachment(channel_id, path)) => {
                                    send_attachment(&mut manager, &store, &channel_id, &path).await;
                                }
                                Some(BackendCommand::SendReaction(channel_id, target_ts, target_sender, emoji)) => {
                                    let self_aci = libsignal_service::protocol::ServiceId::Aci(manager.registration_data().service_ids.aci.into());
                                    let self_name = resolve_sender_name(&store, &self_aci).await;
                                    let display_name = if self_name.contains("Aci") { "You".to_string() } else { self_name };
                                    send_reaction(&mut manager, &channel_id, target_ts, target_sender, &emoji).await;
                                    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                                    App::<FlareApp, AppMessage>::dispatch_main(AppMessage::NewMessage(CoreMessage {
                                        channel_id: Some(channel_id),
                                        sender: self_aci,
                                        sender_name: display_name,
                                        timestamp: ts,
                                        body: Some(emoji),
                                        is_outgoing: true,
                                        is_reaction: true,
                                        reaction_remove: false,
                                        reaction_target_ts: Some(target_ts),
                                        is_receipt: false,
                                        is_call: false,
                                        call_type: None,
                                        is_typing: false,
                                        attachments: vec![],
                                        quote: None,
                                        image_path: None,
                                        video_path: None,
                                        audio_path: None,
                                        file_path: None,
                                    }));
                                }
                                Some(BackendCommand::FetchDevices) => {
                                    fetch_devices(&manager).await;
                                }
                                Some(BackendCommand::SyncContacts) => {
                                    if let Err(e) = manager.request_contacts().await {
                                        log::error!("Failed to sync contacts: {}", e);
                                        App::<FlareApp, AppMessage>::dispatch_main(
                                            AppMessage::BackendActionFailed(format!("Contact sync failed: {}", e)),
                                        );
                                    }
                                }
                                Some(BackendCommand::UnlinkDevice { keep_data }) => {
                                    if let Err(e) = store.clear_registration().await {
                                        log::error!("Failed to clear registration: {}", e);
                                        App::<FlareApp, AppMessage>::dispatch_main(
                                            AppMessage::BackendActionFailed(format!("Unlink failed: {}", e)),
                                        );
                                        continue;
                                    }
                                    if !keep_data {
                                        if let Err(e) = store.clear_contents().await {
                                            log::error!("Failed to clear contents: {}", e);
                                        }
                                    }
                                    App::<FlareApp, AppMessage>::dispatch_main(AppMessage::AppQuit);
                                }
                                Some(BackendCommand::ClearAllMessages) => {
                                    if let Err(e) = store.clear_messages().await {
                                        log::error!("Failed to clear messages: {}", e);
                                        App::<FlareApp, AppMessage>::dispatch_main(
                                            AppMessage::BackendActionFailed(format!("Clear messages failed: {}", e)),
                                        );
                                    } else {
                                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::AppQuit);
                                    }
                                }
                                Some(BackendCommand::ClearChannelMessages(channel_id)) => {
                                    let thread = channel_id_to_thread(&channel_id);
                                    if let Err(e) = store.clear_thread(&thread).await {
                                        log::error!("Failed to clear channel messages: {}", e);
                                        App::<FlareApp, AppMessage>::dispatch_main(
                                            AppMessage::BackendActionFailed(format!("Clear failed: {}", e)),
                                        );
                                    } else {
                                        App::<FlareApp, AppMessage>::dispatch_main(
                                            AppMessage::MessagesLoaded(channel_id, vec![]),
                                        );
                                    }
                                }
                                Some(BackendCommand::OpenAttachment(pointer)) => {
                                    open_attachment(&manager, pointer).await;
                                }
                                Some(BackendCommand::DownloadAttachment(channel_id, timestamp, pointer)) => {
                                    download_attachment(&manager, &store, &channel_id, timestamp, pointer).await;
                                }
                                Some(BackendCommand::DeleteMessage(channel_id, timestamp)) => {
                                    delete_message(&mut store, &channel_id, timestamp).await;
                                }
                                Some(BackendCommand::SubmitCaptcha(token, captcha)) => {
                                    match manager.submit_recaptcha_challenge(&token, &captcha).await {
                                        Ok(()) => {
                                            log::info!("Captcha submitted successfully");
                                        }
                                        Err(e) => {
                                            log::error!("Failed to submit captcha: {}", e);
                                        }
                                    }
                                }
                                None => break,
                            }
                        }
                        complete => break,
                    }
                }
            }
            Err(e) => {
                log::error!("Error receiving messages: {}, retrying in 15s", e);
                loop {
                    select! {
                        cmd = commands.next().fuse() => {
                            match cmd {
                                Some(BackendCommand::LoadMessages(channel_id)) => {
                                    log::trace!("LoadMessages called for channel");
                                    let msgs = load_messages_with_images(&manager, &store, &channel_id, 100, 0).await;
                                    log::trace!("Loaded {} messages, first ts={:?}", msgs.len(), msgs.first().map(|m| m.timestamp));
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::MessagesLoaded(channel_id, msgs),
                                    );
                                }
                                Some(BackendCommand::LoadOlderMessages(channel_id, offset)) => {
                                    let msgs = load_messages_with_images(&manager, &store, &channel_id, 50, offset).await;
                                    App::<FlareApp, AppMessage>::dispatch_main(
                                        AppMessage::OlderMessagesLoaded(msgs),
                                    );
                                }
                                Some(BackendCommand::FetchDevices) => {
                                    fetch_devices(&manager).await;
                                }
                                Some(BackendCommand::SendAttachment(channel_id, path)) => {
                                    send_attachment(&mut manager, &store, &channel_id, &path).await;
                                }
                                Some(BackendCommand::SyncContacts) => {
                                    let _ = manager.request_contacts().await;
                                }
                                Some(BackendCommand::ClearChannelMessages(channel_id)) => {
                                    let thread = channel_id_to_thread(&channel_id);
                                    if store.clear_thread(&thread).await.is_ok() {
                                        App::<FlareApp, AppMessage>::dispatch_main(
                                            AppMessage::MessagesLoaded(channel_id, vec![]),
                                        );
                                    }
                                }
                                Some(BackendCommand::DeleteMessage(channel_id, timestamp)) => {
                                    delete_message(&mut store, &channel_id, timestamp).await;
                                }
                                Some(BackendCommand::SubmitCaptcha(_token, _captcha)) => {
                                    log::warn!("Cannot submit captcha: manager not available");
                                }
                                _ => {}
                            }
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_secs(15)).fuse() => {
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn channel_id_to_thread(channel_id: &ChannelId) -> Thread {
    use libsignal_service::protocol::ServiceId;
    match channel_id {
        ChannelId::Contact(uuid) => Thread::Contact(ServiceId::Aci((*uuid).into())),
        ChannelId::Group(key) => Thread::Group(*key),
    }
}

async fn send_message(
    manager: &mut presage::Manager<Store, Registered>,
    store: &Store,
    channel_id: &ChannelId,
    text: &str,
    quote: Option<crate::core::message::QuoteData>,
) {
    use libsignal_service::content::ContentBody;
    use libsignal_service::protocol::ServiceId;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let quote_proto = quote.as_ref().map(|q| libsignal_service::proto::data_message::Quote {
        id: Some(q.ts),
        text: q.text.clone(),
        ..Default::default()
    });

    let data_message = libsignal_service::proto::DataMessage {
        body: Some(text.to_string()),
        timestamp: Some(timestamp),
        quote: quote_proto,
        ..Default::default()
    };

    let self_aci = ServiceId::Aci(manager.registration_data().service_ids.aci.into());
    let self_name = resolve_sender_name(store, &self_aci).await;
    let display_name = if self_name.contains("Aci") { "You".to_string() } else { self_name };

    let result = match channel_id {
        ChannelId::Contact(uuid) => {
            let recipient = ServiceId::Aci((*uuid).into());
            manager
                .send_message(recipient, ContentBody::DataMessage(data_message), timestamp)
                .await
        }
        ChannelId::Group(key) => {
            manager
                .send_message_to_group(key, ContentBody::DataMessage(data_message), timestamp)
                .await
        }
    };

    match result {
        Ok(_) => {
            let sent_msg = CoreMessage {
                channel_id: Some(channel_id.clone()),
                sender: self_aci,
                sender_name: display_name,
                timestamp,
                body: Some(text.to_string()),
                is_outgoing: true,
                is_reaction: false,
                reaction_remove: false,
                reaction_target_ts: None,
                is_receipt: false,
                is_call: false,
                call_type: None,
                is_typing: false,
                attachments: vec![],
                quote,
                image_path: None,
                video_path: None,
                audio_path: None,
                file_path: None,
            };
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ChannelMessageReceived(
                channel_id.clone(),
                Some(text.to_string()),
                timestamp,
            ));
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::NewMessage(sent_msg));
        }
        Err(e) => {
            log::error!("Failed to send message: {}", e);
        }
    }
}

async fn send_attachment(
    manager: &mut presage::Manager<Store, Registered>,
    store: &Store,
    channel_id: &ChannelId,
    path: &std::path::Path,
) {
    use libsignal_service::content::ContentBody;
    use libsignal_service::protocol::ServiceId;
    use presage::libsignal_service::sender::AttachmentSpec;

    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to read attachment file: {}", e);
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::BackendActionFailed(
                format!("Failed to read file: {}", e),
            ));
            return;
        }
    };

    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string());
    let content_type = content_type_from_path(path);

    let spec = AttachmentSpec {
        content_type: content_type.clone(),
        length: data.len(),
        file_name: file_name.clone(),
        preview: None,
        voice_note: None,
        borderless: None,
        width: None,
        height: None,
        caption: None,
        blur_hash: None,
    };

    let pointers = match manager.upload_attachments(vec![(spec, data)]).await {
        Ok(results) => results,
        Err(e) => {
            log::error!("Failed to upload attachment: {}", e);
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::BackendActionFailed(
                format!("Failed to upload attachment: {}", e),
            ));
            return;
        }
    };

    let attachment_pointers: Vec<_> = pointers.into_iter().filter_map(|r| r.ok()).collect();
    if attachment_pointers.is_empty() {
        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::BackendActionFailed(
            "Attachment upload failed".to_string(),
        ));
        return;
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let data_message = libsignal_service::proto::DataMessage {
        attachments: attachment_pointers.clone(),
        timestamp: Some(timestamp),
        ..Default::default()
    };

    let self_aci = ServiceId::Aci(manager.registration_data().service_ids.aci.into());
    let self_name = resolve_sender_name(store, &self_aci).await;
    let display_name = if self_name.contains("Aci") { "You".to_string() } else { self_name };

    let result = match channel_id {
        ChannelId::Contact(uuid) => {
            let recipient = ServiceId::Aci((*uuid).into());
            manager
                .send_message(recipient, ContentBody::DataMessage(data_message), timestamp)
                .await
        }
        ChannelId::Group(key) => {
            manager
                .send_message_to_group(key, ContentBody::DataMessage(data_message), timestamp)
                .await
        }
    };

    match result {
        Ok(_) => {
            let icon = if content_type.starts_with("image/") { "\u{1f5bc}" }
                else if content_type.starts_with("video/") { "\u{1f3ac}" }
                else if content_type.starts_with("audio/") { "\u{1f3b5}" }
                else { "\u{1f4ce}" };
            let body_text = file_name
                .as_ref()
                .map(|n| format!("{} {}", icon, n))
                .unwrap_or_else(|| format!("{} Attachment", icon));

            // For images, show an inline preview in the message bubble.
            let image_path =
                if content_type.starts_with("image/") && super::preferences_window::download_images() {
                    Some(path.to_path_buf())
                } else {
                    None
                };

            let core_body = if image_path.is_some() {
                // Match receive-side behavior: hide the "[image]" text when we have a preview.
                None
            } else {
                Some(body_text.clone())
            };

            let sent_msg = CoreMessage {
                channel_id: Some(channel_id.clone()),
                sender: self_aci,
                sender_name: display_name,
                timestamp,
                body: core_body,
                is_outgoing: true,
                is_reaction: false,
                reaction_remove: false,
                reaction_target_ts: None,
                is_receipt: false,
                is_call: false,
                call_type: None,
                is_typing: false,
                attachments: attachment_pointers.iter().map(CoreAttachment::from).collect(),
                quote: None,
                image_path,
                video_path: None,
                audio_path: None,
                file_path: None,
            };
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ChannelMessageReceived(
                channel_id.clone(),
                Some(body_text),
                timestamp,
            ));
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::NewMessage(sent_msg));
        }
        Err(e) => {
            log::error!("Failed to send attachment message: {}", e);
        }
    }
}

async fn send_reaction(
    manager: &mut presage::Manager<Store, Registered>,
    channel_id: &ChannelId,
    target_ts: u64,
    target_sender: libsignal_service::protocol::ServiceId,
    emoji: &str,
) {
    use libsignal_service::content::ContentBody;
    use libsignal_service::protocol::ServiceId;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let target_author_aci_binary = match target_sender {
        ServiceId::Aci(aci) => {
            let uuid: libsignal_service::prelude::Uuid = aci.into();
            Some(uuid.as_bytes().to_vec())
        }
        _ => None,
    };

    let data_message = libsignal_service::proto::DataMessage {
        reaction: Some(libsignal_service::proto::data_message::Reaction {
            emoji: Some(emoji.to_string()),
            remove: Some(false),
            target_author_aci_binary,
            target_sent_timestamp: Some(target_ts),
            ..Default::default()
        }),
        timestamp: Some(timestamp),
        ..Default::default()
    };

    let result = match channel_id {
        ChannelId::Contact(uuid) => {
            let recipient = ServiceId::Aci((*uuid).into());
            manager
                .send_message(recipient, ContentBody::DataMessage(data_message), timestamp)
                .await
        }
        ChannelId::Group(key) => {
            manager
                .send_message_to_group(key, ContentBody::DataMessage(data_message), timestamp)
                .await
        }
    };

    if let Err(e) = result {
        log::error!("Failed to send reaction: {}", e);
    }
}

async fn open_attachment(
    manager: &presage::Manager<Store, Registered>,
    pointer: libsignal_service::proto::AttachmentPointer,
) {
    match manager.get_attachment(&pointer).await {
        Err(e) => {
            log::error!("Failed to download attachment: {}", e);
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::BackendActionFailed(
                format!("Download failed: {}", e),
            ));
        }
        Ok(data) => {
            // Determine file extension from content type
            let ext = match pointer.content_type.as_deref().unwrap_or("") {
                ct if ct.starts_with("image/jpeg") => "jpg",
                ct if ct.starts_with("image/png") => "png",
                ct if ct.starts_with("image/gif") => "gif",
                ct if ct.starts_with("image/webp") => "webp",
                ct if ct.starts_with("video/mp4") => "mp4",
                ct if ct.starts_with("video/quicktime") => "mov",
                ct if ct.starts_with("audio/mpeg") => "mp3",
                ct if ct.starts_with("audio/ogg") => "ogg",
                ct if ct.starts_with("audio/") => "m4a",
                ct if ct.starts_with("application/pdf") => "pdf",
                _ => "bin",
            };
            // Prefer original filename, fall back to timestamp-based name
            let filename = pointer.file_name.clone().unwrap_or_else(|| {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                format!("attachment_{}.{}", ts, ext)
            });
            let tmp_path = std::env::temp_dir().join(&filename);
            if let Err(e) = std::fs::write(&tmp_path, &data) {
                log::error!("Failed to write attachment to temp: {}", e);
                App::<FlareApp, AppMessage>::dispatch_main(AppMessage::BackendActionFailed(
                    format!("Could not save attachment: {}", e),
                ));
                return;
            }
            // Open with NSWorkspace
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};
                use std::ffi::CString;
                let path_str = tmp_path.to_string_lossy();
                let cs = CString::new(path_str.as_ref()).unwrap_or_default();
                let ns_str: *mut objc::runtime::Object = msg_send![class!(NSString), stringWithUTF8String: cs.as_ptr()];
                let ns_url: *mut objc::runtime::Object = msg_send![class!(NSURL), fileURLWithPath: ns_str];
                let workspace: *mut objc::runtime::Object = msg_send![class!(NSWorkspace), sharedWorkspace];
                let _: bool = msg_send![workspace, openURL: ns_url];
            }
        }
    }
}

async fn download_attachment(
    manager: &presage::Manager<Store, Registered>,
    store: &Store,
    channel_id: &ChannelId,
    timestamp: u64,
    pointer: libsignal_service::proto::AttachmentPointer,
) {
    let attachment = CoreAttachment::from(&pointer);
    // Determine type and use appropriate cache function
    let cached_path = if is_image_attachment(&attachment) {
        cache_image_attachment(manager, &attachment).await
    } else if is_video_attachment(&attachment) {
        cache_attachment(manager, &attachment).await
    } else if is_audio_attachment(&attachment) {
        cache_attachment(manager, &attachment).await
    } else {
        cache_attachment(manager, &attachment).await
    };

    match cached_path {
        Some(path) => {
            log::info!("Downloaded/cached attachment to: {:?}", path);
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ReloadChannel(channel_id.clone()));
        }
        None => {
            log::error!("Failed to download attachment");
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::BackendActionFailed(
                "Download failed".to_string(),
            ));
        }
    }
}

async fn delete_message(store: &mut Store, channel_id: &ChannelId, timestamp: u64) {
    use presage::store::{ContentsStore, Thread};
    let thread = match channel_id {
        ChannelId::Contact(uuid) => {
            Thread::Contact(libsignal_service::protocol::ServiceId::Aci((*uuid).into()))
        }
        ChannelId::Group(key) => Thread::Group(*key),
    };
    match store.delete_message(&thread, timestamp).await {
        Ok(true) => {
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::MessageDeleted(timestamp));
        }
        Ok(false) => {
            log::warn!("delete_message: message {} not found in store", timestamp);
        }
        Err(e) => {
            log::error!("Failed to delete message {}: {}", timestamp, e);
        }
    }
}

fn content_type_from_path(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("png") => "image/png".to_string(),
        Some("gif") => "image/gif".to_string(),
        Some("webp") => "image/webp".to_string(),
        Some("mp4") => "video/mp4".to_string(),
        Some("mov") => "video/quicktime".to_string(),
        Some("mp3") => "audio/mpeg".to_string(),
        Some("ogg") => "audio/ogg".to_string(),
        Some("aac") => "audio/aac".to_string(),
        Some("m4a") => "audio/mp4".to_string(),
        Some("pdf") => "application/pdf".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

async fn fetch_devices(manager: &presage::Manager<Store, Registered>) {
    match manager.devices().await {
        Ok(device_list) => {
            let entries: Vec<DeviceEntry> = device_list
                .into_iter()
                .map(|d| DeviceEntry {
                    id: u32::from(d.id),
                    name: d.name.unwrap_or_default(),
                    last_seen: d.last_seen.format("%Y-%m-%d %H:%M").to_string(),
                })
                .collect();
            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::DevicesLoaded(entries));
        }
        Err(e) => {
            log::error!("Failed to fetch devices: {}", e);
        }
    }
}

// Attachment image cache

/// Returns the local cache directory for attachment images, creating it if needed.
fn attachment_cache_dir() -> std::path::PathBuf {
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("flare")
        .join("attachments");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Extension from content-type string.
fn ext_for_content_type(ct: &str) -> &'static str {
    if ct.starts_with("image/jpeg") { "jpg" }
    else if ct.starts_with("image/png") { "png" }
    else if ct.starts_with("image/gif") { "gif" }
    else if ct.starts_with("image/webp") { "webp" }
    else { "jpg" }
}

/// Download and cache an image attachment. Returns the local file path.
async fn cache_image_attachment(
    manager: &presage::Manager<Store, Registered>,
    attachment: &crate::core::message::CoreAttachment,
) -> Option<std::path::PathBuf> {
    let digest = attachment.digest.as_ref()?;
    let ct = attachment.content_type.as_deref().unwrap_or("image/jpeg");
    let ext = ext_for_content_type(ct);
    let key = hex::encode(digest);
    let path = attachment_cache_dir().join(format!("{}.{}", key, ext));

    // Already cached
    if path.exists() {
        return Some(path);
    }

    let pointer = libsignal_service::proto::AttachmentPointer {
        content_type: attachment.content_type.clone(),
        file_name: attachment.file_name.clone(),
        size: attachment.size,
        width: attachment.width,
        height: attachment.height,
        blur_hash: attachment.blur_hash.clone(),
        digest: attachment.digest.clone(),
        ..Default::default()
    };

    match manager.get_attachment(&pointer).await {
        Ok(data) => {
            if let Err(e) = std::fs::write(&path, &data) {
                log::warn!("Failed to write attachment cache: {}", e);
                return None;
            }
            Some(path)
        }
        Err(e) => {
            log::warn!("Failed to download attachment: {}", e);
            None
        }
    }
}

/// Returns true if the attachment is a displayable image type.
fn is_image_attachment(attachment: &crate::core::message::CoreAttachment) -> bool {
    attachment.content_type
        .as_deref()
        .map(|ct| ct.starts_with("image/"))
        .unwrap_or(false)
}

/// Returns true if the attachment is a video type.
fn is_video_attachment(attachment: &crate::core::message::CoreAttachment) -> bool {
    attachment.content_type
        .as_deref()
        .map(|ct| ct.starts_with("video/"))
        .unwrap_or(false)
}

/// Returns true if the attachment is an audio type.
fn is_audio_attachment(attachment: &crate::core::message::CoreAttachment) -> bool {
    attachment.content_type
        .as_deref()
        .map(|ct| ct.starts_with("audio/"))
        .unwrap_or(false)
}

/// Download and cache a non-image attachment (video, audio, or file).
async fn cache_attachment(
    manager: &presage::Manager<Store, Registered>,
    attachment: &crate::core::message::CoreAttachment,
) -> Option<std::path::PathBuf> {
    let filename = attachment.file_name.clone().unwrap_or_else(|| "attachment".to_string());
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_string();
    let key = format!("{}_{}", filename, attachment.size.unwrap_or(0));
    let safe_key = key.replace(|c: char| !c.is_alphanumeric(), "_");
    let path = attachment_cache_dir().join(format!("{}.{}", safe_key, ext));
    if path.exists() {
        return Some(path);
    }

    let pointer = libsignal_service::proto::AttachmentPointer {
        content_type: attachment.content_type.clone(),
        file_name: attachment.file_name.clone(),
        size: attachment.size,
        width: attachment.width,
        height: attachment.height,
        blur_hash: attachment.blur_hash.clone(),
        digest: attachment.digest.clone(),
        ..Default::default()
    };

    match manager.get_attachment(&pointer).await {
        Ok(data) => {
            use std::io::Write;
            match std::fs::File::create(&path).and_then(|mut f| f.write_all(&data)) {
                Ok(_) => Some(path),
                Err(e) => {
                    log::warn!("Failed to write attachment cache: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to download attachment: {}", e);
            None
        }
    }
}

/// Load messages and populate `image_path` for image attachments.
pub async fn load_messages_with_images(
    manager: &presage::Manager<Store, Registered>,
    store: &Store,
    channel_id: &ChannelId,
    count: usize,
    offset: usize,
) -> Vec<CoreMessage> {
    let mut messages = load_messages(store, channel_id, count, offset).await;

    // Presage stores our own sent messages as DataMessage with sender = our UUID.
    // CoreMessage::from_content always marks DataMessage as is_outgoing=false, so we
    // need to detect and fix outgoing messages here.
    let my_uuid: libsignal_service::prelude::Uuid =
        manager.registration_data().service_ids.aci.into();
    for msg in &mut messages {
        if let libsignal_service::protocol::ServiceId::Aci(aci) = msg.sender {
            let sender_uuid: libsignal_service::prelude::Uuid = aci.into();
            if sender_uuid == my_uuid && !msg.is_outgoing {
                msg.is_outgoing = true;
                msg.sender_name = String::new();
            }
        }
    }

    let download_images = super::preferences_window::download_images();
    let download_videos = super::preferences_window::download_videos();
    let download_audio = super::preferences_window::download_voice();
    let download_files = super::preferences_window::download_files();

    for msg in &mut messages {
        // Cache image attachments
        if download_images {
            if let Some(ptr) = msg.attachments.iter().find(|p| is_image_attachment(p)) {
                msg.image_path = cache_image_attachment(manager, ptr).await;
                // Remove the emoji placeholder text for pure-image messages
                if msg.image_path.is_some() {
                    if let Some(ref b) = msg.body.clone() {
                        if b.starts_with('\u{1f5bc}') || b.starts_with('\u{1f3ac}') {
                            msg.body = None;
                        }
                    }
                }
            }
        }
        // Cache video attachments
        if download_videos {
            if let Some(ptr) = msg.attachments.iter().find(|p| is_video_attachment(p)) {
                msg.video_path = cache_attachment(manager, ptr).await;
                if msg.video_path.is_some() {
                    msg.body = None;
                }
            }
        }
        // Cache audio attachments
        if download_audio {
            if let Some(ptr) = msg.attachments.iter().find(|p| is_audio_attachment(p)) {
                msg.audio_path = cache_attachment(manager, ptr).await;
                if msg.audio_path.is_some() {
                    msg.body = None;
                }
            }
        }
        // Cache file attachments (non-image, non-video, non-audio)
        if download_files {
            if msg.image_path.is_none() && msg.video_path.is_none() && msg.audio_path.is_none() {
                if let Some(ptr) = msg.attachments.first() {
                    if !is_image_attachment(ptr) && !is_video_attachment(ptr) && !is_audio_attachment(ptr) {
                        msg.file_path = cache_attachment(manager, ptr).await;
                        if msg.file_path.is_some() {
                            msg.body = None;
                        }
                    }
                }
            }
        }
    }
    messages
}
