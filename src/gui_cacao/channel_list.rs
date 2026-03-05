use std::cell::RefCell;
use std::sync::Arc;

use cacao::color::Color;
use cacao::input::{TextField, TextFieldDelegate};
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::objc_access::ObjcAccess;
use cacao::text::{Font, Label, LineBreakMode, TextAlign};
use cacao::view::{View, ViewDelegate};

use crate::core::channel::CoreChannel;

use super::app::{AppMessage, FlareApp};
use super::backend::{BackendCommand, BackendState};
use cacao::appkit::App;

const CHANNEL_ROW: &str = "ChannelRowCell";

fn avatar_color_for(title: &str) -> Color {
    let mut hash: u32 = 5381;
    for b in title.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u32);
    }
    match hash % 6 {
        0 => Color::SystemBlue,
        1 => Color::SystemGreen,
        2 => Color::SystemPurple,
        3 => Color::SystemOrange,
        4 => Color::SystemTeal,
        _ => Color::SystemIndigo,
    }
}

fn initials_for(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().collect();
    match words.len() {
        0 => "?".to_string(),
        1 => words[0]
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default(),
        _ => {
            let a = words[0]
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            let b = words[words.len() - 1]
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            format!("{}{}", a, b)
        }
    }
}

// — Search field delegate —

#[derive(Debug, Default)]
pub struct SearchFieldDelegate;

impl TextFieldDelegate for SearchFieldDelegate {
    const NAME: &'static str = "ChannelSearchField";

    fn text_did_change(&self, value: &str) {
        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::SearchChanged(value.to_string()));
    }

    fn text_did_end_editing(&self, _value: &str) {}
}

pub type SearchField = TextField<SearchFieldDelegate>;

pub fn new_search_field() -> SearchField {
    let field = TextField::with(SearchFieldDelegate);
    field
}

// — Channel row —

#[derive(Default, Debug)]
pub struct ChannelRow {
    pub avatar: View,
    pub initials: Label,
    pub title: Label,
    pub timestamp: Label,
    pub subtitle: Label,
    pub badge: Label,
}

impl ChannelRow {
    pub fn configure_with(&mut self, channel: &CoreChannel) {
        let color = avatar_color_for(&channel.title);
        self.avatar.set_background_color(color);
        self.avatar.layer.set_corner_radius(18.0);

        self.initials.set_text(&initials_for(&channel.title));
        self.title.set_text(&channel.title);
        self.subtitle
            .set_text(channel.last_message_text.as_deref().unwrap_or(""));

        if let Some(ts) = channel.last_message_timestamp {
            let secs = ts / 1000;
            let time_str = chrono::DateTime::from_timestamp(secs as i64, 0)
                .map(|dt| {
                    let now = chrono::Utc::now();
                    if dt.date_naive() == now.date_naive() {
                        dt.format("%H:%M").to_string()
                    } else {
                        dt.format("%b %d").to_string()
                    }
                })
                .unwrap_or_default();
            self.timestamp.set_text(&time_str);
        } else {
            self.timestamp.set_text("");
        }

        if channel.unread_count > 0 {
            self.badge.set_text(&channel.unread_count.to_string());
            self.badge.set_hidden(false);
            self.badge.layer.set_corner_radius(9.0);
        } else {
            self.badge.set_hidden(true);
        }
    }
}

impl ViewDelegate for ChannelRow {
    const NAME: &'static str = "ChannelRow";

    fn did_load(&mut self, view: View) {
        view.add_subview(&self.avatar);
        view.add_subview(&self.initials);
        view.add_subview(&self.title);
        view.add_subview(&self.timestamp);
        view.add_subview(&self.subtitle);
        view.add_subview(&self.badge);

        self.initials.set_font(&Font::bold_system(14.));
        self.initials.set_text_color(Color::SystemWhite);
        self.initials.set_text_alignment(TextAlign::Center);

        self.title.set_line_break_mode(LineBreakMode::TruncateTail);
        self.subtitle
            .set_line_break_mode(LineBreakMode::TruncateTail);
        self.subtitle.set_text_color(Color::SystemGray);
        self.subtitle.set_font(&Font::system(12.));
        self.timestamp.set_font(&Font::system(11.));
        self.timestamp.set_text_color(Color::SystemGray);

        self.badge.set_font(&Font::bold_system(11.));
        self.badge.set_text_color(Color::SystemWhite);
        self.badge.set_text_alignment(TextAlign::Center);
        self.badge.set_background_color(Color::SystemBlue);

        LayoutConstraint::activate(&[
            self.avatar
                .leading
                .constraint_equal_to(&view.leading)
                .offset(8.),
            self.avatar.center_y.constraint_equal_to(&view.center_y),
            self.avatar.width.constraint_equal_to_constant(36.),
            self.avatar.height.constraint_equal_to_constant(36.),
            self.initials
                .center_x
                .constraint_equal_to(&self.avatar.center_x),
            self.initials
                .center_y
                .constraint_equal_to(&self.avatar.center_y),
            self.title.top.constraint_equal_to(&view.top).offset(8.),
            self.title
                .leading
                .constraint_equal_to(&self.avatar.trailing)
                .offset(8.),
            self.title
                .trailing
                .constraint_less_than_or_equal_to(&self.timestamp.leading)
                .offset(-4.),
            self.timestamp
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            self.timestamp
                .center_y
                .constraint_equal_to(&self.title.center_y),
            self.timestamp
                .width
                .constraint_greater_than_or_equal_to_constant(0.),
            self.subtitle
                .top
                .constraint_equal_to(&self.title.bottom)
                .offset(2.),
            self.subtitle
                .leading
                .constraint_equal_to(&self.avatar.trailing)
                .offset(8.),
            self.subtitle
                .trailing
                .constraint_less_than_or_equal_to(&self.badge.leading)
                .offset(-4.),
            self.subtitle
                .bottom
                .constraint_equal_to(&view.bottom)
                .offset(-8.),
            self.badge
                .center_y
                .constraint_equal_to(&self.subtitle.center_y),
            self.badge
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            self.badge
                .width
                .constraint_greater_than_or_equal_to_constant(18.),
            self.badge.height.constraint_equal_to_constant(18.),
        ]);
    }
}

// — List delegate —

pub struct ChannelListDelegate {
    view: Option<ListView>,
    all_channels: RefCell<Vec<CoreChannel>>,
    visible_channels: RefCell<Vec<CoreChannel>>,
    backend_state: Arc<BackendState>,
    /// Channel currently open in the message pane (used to restore selection after reload).
    active_channel_id: RefCell<Option<crate::core::channel::ChannelId>>,
    /// True while we are programmatically reselecting a row; suppresses item_selected.
    reselecting: RefCell<bool>,
}

impl std::fmt::Debug for ChannelListDelegate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelListDelegate").finish()
    }
}

impl ChannelListDelegate {
    pub fn new(backend_state: Arc<BackendState>) -> Self {
        Self {
            view: None,
            all_channels: RefCell::new(Vec::new()),
            visible_channels: RefCell::new(Vec::new()),
            backend_state,
            active_channel_id: RefCell::new(None),
            reselecting: RefCell::new(false),
        }
    }

    pub fn set_channels(&self, mut channels: Vec<CoreChannel>) {
        // Merge: preserve last_message data and unread counts from existing channels
        // that the new list may not have (e.g. channels created from incoming messages
        // before contacts synced).
        let existing = self.all_channels.borrow();
        for old in existing.iter() {
            if let Some(new_ch) = channels.iter_mut().find(|c| c.id == old.id) {
                // Keep the most recent message info
                if new_ch.last_message_timestamp < old.last_message_timestamp {
                    new_ch.last_message_text = old.last_message_text.clone();
                    new_ch.last_message_timestamp = old.last_message_timestamp;
                }
                new_ch.unread_count = new_ch.unread_count.max(old.unread_count);
            } else {
                // Channel existed from messages but isn't in the new contacts list yet
                // (e.g. message from unknown sender) — keep it
                channels.push(old.clone());
            }
        }
        drop(existing);
        channels.sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));
        let visible = channels.clone();
        *self.all_channels.borrow_mut() = channels;
        *self.visible_channels.borrow_mut() = visible;
        if let Some(view) = &self.view {
            view.reload();
        }
        self.reselect_active();
    }

    /// Programmatically select channel at 0-based index in the visible list.
    pub fn activate_channel_at_index(&self, index: usize) {
        let channel = self.visible_channels.borrow().get(index).cloned();
        if let Some(channel) = channel {
            *self.active_channel_id.borrow_mut() = Some(channel.id.clone());
            if let Some(tx) = self.backend_state.command_tx.lock().unwrap().as_ref() {
                let _ = tx.unbounded_send(BackendCommand::LoadMessages(channel.id));
            }
            // Visually select the row in the table
            if let Some(ref view) = self.view {
                let ptr = view
                    .objc
                    .get(|obj| obj as *const _ as *mut objc::runtime::Object);
                unsafe {
                    use objc::{class, msg_send, sel, sel_impl};
                    let index_set: *mut objc::runtime::Object =
                        msg_send![class!(NSIndexSet), indexSetWithIndex: index];
                    let _: () = msg_send![ptr, selectRowIndexes: index_set byExtendingSelection: objc::runtime::NO];
                }
            }
        }
    }

    /// Mark the channel as active (called when messages are loaded for a channel).
    pub fn set_active_channel(&self, id: &crate::core::channel::ChannelId) {
        *self.active_channel_id.borrow_mut() = Some(id.clone());
        self.reselect_active();
    }

    /// Restore visual selection to the active channel without sending LoadMessages.
    fn reselect_active(&self) {
        let active = self.active_channel_id.borrow().clone();
        if let Some(ref id) = active {
            let index = self
                .visible_channels
                .borrow()
                .iter()
                .position(|c| &c.id == id);
            if let (Some(idx), Some(view)) = (index, &self.view) {
                let ptr = view
                    .objc
                    .get(|obj| obj as *const _ as *mut objc::runtime::Object);
                *self.reselecting.borrow_mut() = true;
                unsafe {
                    use objc::{class, msg_send, sel, sel_impl};
                    let index_set: *mut objc::runtime::Object =
                        msg_send![class!(NSIndexSet), indexSetWithIndex: idx];
                    let _: () = msg_send![ptr, selectRowIndexes: index_set byExtendingSelection: objc::runtime::NO];
                }
                *self.reselecting.borrow_mut() = false;
            }
        }
    }

    pub fn find_channel(&self, id: &crate::core::channel::ChannelId) -> Option<CoreChannel> {
        self.all_channels
            .borrow()
            .iter()
            .find(|c| &c.id == id)
            .cloned()
    }

    pub fn all_channels(&self) -> Vec<CoreChannel> {
        self.all_channels.borrow().clone()
    }

    pub fn update_channel_last_message(
        &self,
        id: &crate::core::channel::ChannelId,
        body: Option<String>,
        timestamp: u64,
    ) {
        let mut channels = self.all_channels.borrow_mut();
        if let Some(ch) = channels.iter_mut().find(|c| &c.id == id) {
            ch.last_message_text = body;
            ch.last_message_timestamp = Some(timestamp);
        } else {
            // Channel not yet known (e.g. fresh link before contacts sync).
            // Create a placeholder so it appears in the sidebar immediately.
            let title = match id {
                crate::core::channel::ChannelId::Contact(uuid) => format!("{}", uuid),
                crate::core::channel::ChannelId::Group(_) => "Group".to_string(),
            };
            channels.push(CoreChannel {
                id: id.clone(),
                title,
                is_group: matches!(id, crate::core::channel::ChannelId::Group(_)),
                last_message_text: body,
                last_message_timestamp: Some(timestamp),
                unread_count: 0,
                members: Vec::new(),
            });
        }
        channels.sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));
        drop(channels);
        let visible: Vec<CoreChannel> = self.all_channels.borrow().clone();
        *self.visible_channels.borrow_mut() = visible;
        if let Some(view) = &self.view {
            view.reload();
        }
        self.reselect_active();
    }

    pub fn increment_unread(&self, id: &crate::core::channel::ChannelId) {
        let mut channels = self.all_channels.borrow_mut();
        if let Some(ch) = channels.iter_mut().find(|c| &c.id == id) {
            ch.unread_count += 1;
        } else {
            let title = match id {
                crate::core::channel::ChannelId::Contact(uuid) => format!("{}", uuid),
                crate::core::channel::ChannelId::Group(_) => "Group".to_string(),
            };
            channels.push(CoreChannel {
                id: id.clone(),
                title,
                is_group: matches!(id, crate::core::channel::ChannelId::Group(_)),
                last_message_text: None,
                last_message_timestamp: None,
                unread_count: 1,
                members: Vec::new(),
            });
        }
        drop(channels);
        let visible: Vec<CoreChannel> = self.all_channels.borrow().clone();
        *self.visible_channels.borrow_mut() = visible;
        if let Some(view) = &self.view {
            view.reload();
        }
        self.reselect_active();
    }

    pub fn clear_unread(&self, id: &crate::core::channel::ChannelId) {
        let mut channels = self.all_channels.borrow_mut();
        if let Some(ch) = channels.iter_mut().find(|c| &c.id == id) {
            ch.unread_count = 0;
        }
        drop(channels);
        let visible: Vec<CoreChannel> = self.all_channels.borrow().clone();
        *self.visible_channels.borrow_mut() = visible;
        if let Some(view) = &self.view {
            view.reload();
        }
        self.reselect_active();
    }

    pub fn set_filter(&self, query: &str) {
        let query_lc = query.to_lowercase();
        let filtered: Vec<CoreChannel> = self
            .all_channels
            .borrow()
            .iter()
            .filter(|c| c.title.to_lowercase().contains(&query_lc))
            .cloned()
            .collect();
        *self.visible_channels.borrow_mut() = filtered;
        if let Some(view) = &self.view {
            view.reload();
        }
    }
}

impl ListViewDelegate for ChannelListDelegate {
    const NAME: &'static str = "ChannelListView";

    fn did_load(&mut self, view: ListView) {
        view.register(CHANNEL_ROW, ChannelRow::default);
        view.set_row_height(48.);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.visible_channels.borrow().len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let mut view = self
            .view
            .as_ref()
            .unwrap()
            .dequeue::<ChannelRow>(CHANNEL_ROW);
        if let Some(delegate) = &mut view.delegate {
            let channels = self.visible_channels.borrow();
            if let Some(channel) = channels.get(row) {
                delegate.configure_with(channel);
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        if *self.reselecting.borrow() {
            return;
        }
        if let Some(row) = row {
            let channels = self.visible_channels.borrow();
            if let Some(channel) = channels.get(row) {
                log::trace!("Selected channel: {}", channel.title);
                let channel_id = channel.id.clone();
                *self.active_channel_id.borrow_mut() = Some(channel_id.clone());
                if let Some(tx) = self.backend_state.command_tx.lock().unwrap().as_ref() {
                    let _ = tx.unbounded_send(BackendCommand::LoadMessages(channel_id));
                }
            }
        }
    }
}
