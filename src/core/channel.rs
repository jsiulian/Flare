use presage::model::groups::Group;
use presage::store::{ContentsStore, Thread};
use presage_store_sqlite::SqliteStore as Store;

use super::message::CoreMessage;

/// A lightweight channel representation for the cacao GUI.
/// Represents either a 1:1 contact conversation or a group.
#[derive(Debug, Clone)]
pub struct CoreChannel {
    pub id: ChannelId,
    pub title: String,
    pub is_group: bool,
    pub last_message_text: Option<String>,
    pub last_message_timestamp: Option<u64>,
    pub unread_count: u32,
    /// Display names of group members (empty for DMs).
    pub members: Vec<String>,
}

/// Unique identifier for a channel — either a contact UUID or a group master key.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ChannelId {
    Contact(libsignal_service::prelude::Uuid),
    Group([u8; 32]),
}

/// Load the most recent displayable message for a thread.
async fn last_message_for_thread(store: &Store, thread: &Thread) -> Option<CoreMessage> {
    match store.messages(thread, ..).await {
        Ok(messages) => messages
            .filter_map(|r| r.ok())
            .filter_map(|content| CoreMessage::from_content(&content))
            .filter(|m| !m.is_receipt && !m.is_typing && !m.is_reaction)
            .max_by_key(|m| m.timestamp),
        Err(_) => None,
    }
}

/// Load all channels (contacts + groups) from the presage store.
pub async fn load_channels(store: &Store) -> Vec<CoreChannel> {
    let mut channels = Vec::new();

    // Load contacts
    if let Ok(contacts_iter) = store.contacts().await {
        for contact in contacts_iter.filter_map(|c| c.ok()) {
            let uuid = contact.uuid;
            let title = if !contact.name.is_empty() {
                contact.name.clone()
            } else if let Some(ref phone) = contact.phone_number {
                phone.to_string()
            } else {
                format!("{}", uuid)
            };

            let thread = Thread::Contact(libsignal_service::protocol::ServiceId::Aci(uuid.into()));
            let last = last_message_for_thread(store, &thread).await;

            channels.push(CoreChannel {
                id: ChannelId::Contact(uuid),
                title,
                is_group: false,
                last_message_text: last.as_ref().and_then(|m| m.body.clone()),
                last_message_timestamp: last.map(|m| m.timestamp),
                unread_count: 0,
                members: Vec::new(),
            });
        }
    }

    // Load groups
    if let Ok(groups_iter) = store.groups().await {
        for item in groups_iter.filter_map(|g| g.ok()) {
            let (key, group): ([u8; 32], Group) = item;
            let title = group.title.clone();

            // Resolve member display names from contacts store
            let mut member_names: Vec<String> = Vec::new();
            if let Ok(contacts_iter) = store.contacts().await {
                let contacts: Vec<_> = contacts_iter.filter_map(|c| c.ok()).collect();
                for member in &group.members {
                    let uuid: libsignal_service::prelude::Uuid = member.aci.into();
                    let name = contacts
                        .iter()
                        .find(|c| c.uuid == uuid)
                        .map(|c| if !c.name.is_empty() { c.name.clone() } else { uuid.to_string() })
                        .unwrap_or_else(|| uuid.to_string());
                    member_names.push(name);
                }
            }

            let thread = Thread::Group(key);
            let last = last_message_for_thread(store, &thread).await;

            channels.push(CoreChannel {
                id: ChannelId::Group(key),
                title,
                is_group: true,
                last_message_text: last.as_ref().and_then(|m| m.body.clone()),
                last_message_timestamp: last.map(|m| m.timestamp),
                unread_count: 0,
                members: member_names,
            });
        }
    }

    // Sort by most recent activity
    channels.sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));

    channels
}
