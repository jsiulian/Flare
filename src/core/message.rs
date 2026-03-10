use libsignal_service::prelude::Content;
use libsignal_service::protocol::ServiceId;
use presage::store::{ContentsStore, Thread};
use presage_store_sqlite::SqliteStore as Store;
use std::cmp::Reverse;

use super::channel::ChannelId;

fn format_attachment(att: &libsignal_service::proto::AttachmentPointer) -> String {
    let name = att.file_name.as_deref().unwrap_or("");
    let ct = att.content_type.as_deref().unwrap_or("");
    let icon = if ct.starts_with("image/") { "🖼" }
        else if ct.starts_with("video/") { "🎬" }
        else if ct.starts_with("audio/") { "🎵" }
        else { "📎" };
    if !name.is_empty() { format!("{} {}", icon, name) }
    else { format!("{} Attachment", icon) }
}

fn format_attachments(atts: &[libsignal_service::proto::AttachmentPointer]) -> String {
    atts.iter().map(format_attachment).collect::<Vec<_>>().join("\n")
}

#[derive(Debug, Clone)]
pub struct QuoteData {
    pub ts: u64,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CallType {
    Offer,    // Incoming/outgoing call
    Answer,   // Call started
    Hangup,   // Call ended
    Busy,     // Declined/missed
}

impl CallType {
    pub fn from_call_message(cm: &libsignal_service::content::CallMessage) -> Option<Self> {
        if cm.offer.is_some() {
            Some(CallType::Offer)
        } else if cm.hangup.is_some() {
            Some(CallType::Hangup)
        } else if cm.answer.is_some() {
            Some(CallType::Answer)
        } else if cm.busy.is_some() {
            Some(CallType::Busy)
        } else {
            None
        }
    }

    pub fn display_text(&self, is_outgoing: bool) -> String {
        match self {
            CallType::Offer => {
                if is_outgoing { "Outgoing call".to_string() } else { "Incoming call".to_string() }
            }
            CallType::Answer => "Call started".to_string(),
            CallType::Hangup => "Call ended".to_string(),
            CallType::Busy => {
                if is_outgoing { "Unanswered call".to_string() } else { "Call declined".to_string() }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoreMessage {
    pub channel_id: Option<ChannelId>,
    pub sender: ServiceId,
    pub sender_name: String,
    pub timestamp: u64,
    pub body: Option<String>,
    pub is_outgoing: bool,
    pub is_reaction: bool,
    pub reaction_target_ts: Option<u64>,
    pub is_receipt: bool,
    pub is_call: bool,
    pub call_type: Option<CallType>,
    pub is_typing: bool,
    pub reaction_remove: bool,
    pub attachments: Vec<libsignal_service::proto::AttachmentPointer>,
    pub quote: Option<QuoteData>,
    /// Path to a locally-cached image file for the first image attachment, if downloaded.
    pub image_path: Option<std::path::PathBuf>,
    /// Path to a locally-cached video file, if downloaded.
    pub video_path: Option<std::path::PathBuf>,
    /// Path to a locally-cached audio file, if downloaded.
    pub audio_path: Option<std::path::PathBuf>,
    /// Path to a locally-cached generic file, if downloaded.
    pub file_path: Option<std::path::PathBuf>,
}

fn channel_id_from_data(
    sender: ServiceId,
    group_v2: &Option<libsignal_service::proto::GroupContextV2>,
) -> Option<ChannelId> {
    if let Some(gv2) = group_v2 {
        if let Some(ref key_bytes) = gv2.master_key {
            if key_bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(key_bytes);
                return Some(ChannelId::Group(key));
            }
        }
    }
    if let ServiceId::Aci(aci) = sender {
        let uuid: libsignal_service::prelude::Uuid = aci.into();
        return Some(ChannelId::Contact(uuid));
    }
    None
}

impl CoreMessage {
    pub fn from_content(content: &Content) -> Option<Self> {
        use libsignal_service::content::ContentBody;
        use libsignal_service::proto::SyncMessage;
        let sender = content.metadata.sender;
        let timestamp = content.metadata.timestamp;
        let sender_name = format!("{:?}", sender);

        match &content.body {
            ContentBody::DataMessage(dm) => {
                let channel_id = channel_id_from_data(sender, &dm.group_v2);
                if dm.reaction.is_some() {
                    let r = dm.reaction.as_ref().unwrap();
                    return Some(Self {
                        channel_id,
                        sender,
                        sender_name,
                        timestamp,
                        body: r.emoji.clone(),
                        is_outgoing: false,
                        is_reaction: true,
                        reaction_remove: r.remove.unwrap_or(false),
                        reaction_target_ts: r.target_sent_timestamp,
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
                    });
                }
                let quote = dm.quote.as_ref().and_then(|q| {
                    let ts = q.id?;
                    Some(QuoteData { ts, text: q.text.clone() })
                });
                let body = if !dm.attachments.is_empty() {
                    let att_text = format_attachments(&dm.attachments);
                    if let Some(text) = &dm.body {
                        Some(format!("{}\n{}", text, att_text))
                    } else {
                        Some(att_text)
                    }
                } else if let Some(text) = &dm.body {
                    Some(text.clone())
                } else if dm.sticker.is_some() {
                    Some("[Sticker]".to_string())
                } else {
                    None
                };
                Some(Self {
                    channel_id,
                    sender,
                    sender_name,
                    timestamp,
                    body,
                    is_outgoing: false,
                    is_reaction: false,
                    reaction_remove: false,
                    reaction_target_ts: None,
                    is_receipt: false,
                    is_call: false,
                    call_type: None,
                    is_typing: false,
                    attachments: dm.attachments.clone(),
                    quote,
                    image_path: None,
                    video_path: None,
                    audio_path: None,
                    file_path: None,
                })
            }
            ContentBody::SynchronizeMessage(SyncMessage { sent: Some(sent), .. }) => {
                if let Some(ref dm) = sent.message {
                    let channel_id = if dm.group_v2.is_some() {
                        channel_id_from_data(sender, &dm.group_v2)
                    } else if let Some(dest) = sent.parse_destination_service_id() {
                        if let ServiceId::Aci(aci) = dest {
                            let uuid: libsignal_service::prelude::Uuid = aci.into();
                            Some(ChannelId::Contact(uuid))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let sync_quote = if dm.reaction.is_none() {
                        dm.quote.as_ref().and_then(|q| {
                            let ts = q.id?;
                            Some(QuoteData { ts, text: q.text.clone() })
                        })
                    } else {
                        None
                    };
                    let body = if dm.reaction.is_some() {
                        dm.reaction.as_ref().and_then(|r| r.emoji.clone())
                    } else if !dm.attachments.is_empty() {
                        let att_text = format_attachments(&dm.attachments);
                        if let Some(text) = &dm.body {
                            Some(format!("{}\n{}", text, att_text))
                        } else {
                            Some(att_text)
                        }
                    } else if let Some(text) = &dm.body {
                        Some(text.clone())
                    } else if dm.sticker.is_some() {
                        Some("[Sticker]".to_string())
                    } else {
                        None
                    };
                    let reaction_target_ts = dm.reaction.as_ref()
                        .and_then(|r| r.target_sent_timestamp);
                    let reaction_remove = dm.reaction.as_ref()
                        .and_then(|r| r.remove)
                        .unwrap_or(false);
                    return Some(Self {
                        channel_id,
                        sender,
                        sender_name,
                        timestamp,
                        body,
                        is_outgoing: true,
                        is_reaction: dm.reaction.is_some(),
                        reaction_remove,
                        reaction_target_ts,
                        is_receipt: false,
                        is_call: false,
                        call_type: None,
                        is_typing: false,
                        attachments: dm.attachments.clone(),
                        quote: sync_quote,
                        image_path: None,
                        video_path: None,
                        audio_path: None,
                        file_path: None,
                    });
                }
                None
            }
            ContentBody::CallMessage(cm) => {
                let channel_id = if let ServiceId::Aci(aci) = sender {
                    let uuid: libsignal_service::prelude::Uuid = aci.into();
                    Some(ChannelId::Contact(uuid))
                } else {
                    None
                };
                let call_type = CallType::from_call_message(cm);
                let body = call_type.as_ref().map(|ct| ct.display_text(false));
                Some(Self {
                    channel_id,
                    sender,
                    sender_name,
                    timestamp,
                    body,
                    is_outgoing: false,
                    is_reaction: false,
                    reaction_remove: false,
                    reaction_target_ts: None,
                    is_receipt: false,
                    is_call: true,
                    call_type,
                    is_typing: false,
                    attachments: vec![],
                    quote: None,
                    image_path: None,
                    video_path: None,
                    audio_path: None,
                    file_path: None,
                })
            }
            ContentBody::TypingMessage(tm) => {
                let channel_id = if let Some(group_id) = &tm.group_id {
                    if group_id.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(group_id);
                        Some(ChannelId::Group(key))
                    } else {
                        None
                    }
                } else if let ServiceId::Aci(aci) = sender {
                    let uuid: libsignal_service::prelude::Uuid = aci.into();
                    Some(ChannelId::Contact(uuid))
                } else {
                    None
                };
                Some(Self {
                    channel_id,
                    sender,
                    sender_name,
                    timestamp,
                    body: None,
                    is_outgoing: false,
                    is_reaction: false,
                    reaction_remove: false,
                    reaction_target_ts: None,
                    is_receipt: false,
                    is_call: false,
                    call_type: None,
                    is_typing: true,
                    attachments: vec![],
                    quote: None,
                    image_path: None,
                    video_path: None,
                    audio_path: None,
                    file_path: None,
                })
            }
            ContentBody::ReceiptMessage(_) => {
                let channel_id = if let ServiceId::Aci(aci) = sender {
                    let uuid: libsignal_service::prelude::Uuid = aci.into();
                    Some(ChannelId::Contact(uuid))
                } else {
                    None
                };
                Some(Self {
                    channel_id,
                    sender,
                    sender_name,
                    timestamp,
                    body: None,
                    is_outgoing: false,
                    is_reaction: false,
                    reaction_remove: false,
                    reaction_target_ts: None,
                    is_receipt: true,
                    is_call: false,
                    call_type: None,
                    is_typing: false,
                    attachments: vec![],
                    quote: None,
                    image_path: None,
                    video_path: None,
                    audio_path: None,
                    file_path: None,
                })
            }
            _ => None,
        }
    }
}

fn thread_for_channel(channel_id: &ChannelId) -> Thread {
    match channel_id {
        ChannelId::Contact(uuid) => Thread::Contact(ServiceId::Aci((*uuid).into())),
        ChannelId::Group(key) => Thread::Group(*key),
    }
}

pub async fn resolve_sender_name(store: &Store, sender: &ServiceId) -> String {
    if let ServiceId::Aci(aci) = sender {
        let uuid: libsignal_service::prelude::Uuid = (*aci).into();
        if let Ok(contacts) = store.contacts().await {
            for contact in contacts.flatten() {
                if contact.uuid == uuid && !contact.name.is_empty() {
                    return contact.name;
                }
            }
        }
    }
    format!("{:?}", sender)
}

pub async fn load_messages(
    store: &Store,
    channel_id: &ChannelId,
    count: usize,
    offset: usize,
) -> Vec<CoreMessage> {
    let thread = thread_for_channel(channel_id);
    let mut messages = match store.messages(&thread, ..).await {
        Ok(messages) => {
            let mut msgs: Vec<_> = messages
                .filter_map(|r| r.ok())
                .filter_map(|content| CoreMessage::from_content(&content))
                .filter(|m| !m.is_receipt && !m.is_typing)
                .collect();
            msgs.sort_by_key(|m| Reverse(m.timestamp));
            msgs.into_iter().skip(offset).take(count).collect()
        }
        Err(e) => {
            log::error!("Failed to load messages: {:?}", e);
            Vec::new()
        }
    };

    for msg in &mut messages {
        msg.sender_name = resolve_sender_name(store, &msg.sender).await;
    }

    messages
}
