use std::cell::RefCell;
use std::sync::Arc;

use cacao::appkit::App;
use cacao::appkit::window::{Window, WindowConfig, WindowDelegate, WindowStyle};
use cacao::color::Color;
use cacao::geometry::Rect;
use cacao::input::{TextField, TextFieldDelegate};
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::text::{Font, Label};
use cacao::view::{View, ViewDelegate};

use crate::core::channel::CoreChannel;

use super::app::{AppMessage, FlareApp};
use super::backend::{BackendCommand, BackendState};

const CONTACT_ROW: &str = "ContactPickerRow";

// — Search field delegate —

#[derive(Debug, Default)]
pub struct ContactSearchFieldDelegate;

impl TextFieldDelegate for ContactSearchFieldDelegate {
    const NAME: &'static str = "ContactSearchField";

    fn text_did_change(&self, value: &str) {
        App::<FlareApp, AppMessage>::dispatch_main(
            AppMessage::ContactPickerSearch(value.to_string()),
        );
    }

    fn text_did_end_editing(&self, _value: &str) {}
}

pub type ContactSearchField = TextField<ContactSearchFieldDelegate>;

fn new_contact_search_field() -> ContactSearchField {
    TextField::with(ContactSearchFieldDelegate)
}

// — Row —

#[derive(Default, Debug)]
pub struct ContactPickerRow {
    title: Label,
}

impl ViewDelegate for ContactPickerRow {
    const NAME: &'static str = "ContactPickerRow";

    fn did_load(&mut self, view: View) {
        view.add_subview(&self.title);
        self.title.set_font(&Font::system(14.));

        LayoutConstraint::activate(&[
            self.title.top.constraint_equal_to(&view.top).offset(8.),
            self.title.bottom.constraint_equal_to(&view.bottom).offset(-8.),
            self.title.leading.constraint_equal_to(&view.leading).offset(12.),
            self.title.trailing.constraint_equal_to(&view.trailing).offset(-12.),
        ]);
    }
}

// — List delegate —

pub struct ContactListDelegate {
    view: Option<ListView>,
    all_contacts: RefCell<Vec<CoreChannel>>,
    filtered: RefCell<Vec<CoreChannel>>,
    backend_state: Arc<BackendState>,
}

impl std::fmt::Debug for ContactListDelegate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContactListDelegate").finish()
    }
}

impl ContactListDelegate {
    fn new(backend_state: Arc<BackendState>) -> Self {
        Self {
            view: None,
            all_contacts: RefCell::new(Vec::new()),
            filtered: RefCell::new(Vec::new()),
            backend_state,
        }
    }

    pub fn set_contacts(&self, contacts: Vec<CoreChannel>) {
        *self.filtered.borrow_mut() = contacts.clone();
        *self.all_contacts.borrow_mut() = contacts;
        if let Some(v) = &self.view {
            v.reload();
        }
    }

    pub fn set_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        let all = self.all_contacts.borrow();
        if q.is_empty() {
            *self.filtered.borrow_mut() = all.clone();
        } else {
            *self.filtered.borrow_mut() = all
                .iter()
                .filter(|c| c.title.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        if let Some(v) = &self.view {
            v.reload();
        }
        self.filtered.borrow().is_empty()
    }
}

impl ListViewDelegate for ContactListDelegate {
    const NAME: &'static str = "ContactListView";

    fn did_load(&mut self, view: ListView) {
        view.register(CONTACT_ROW, ContactPickerRow::default);
        view.set_uses_automatic_row_heights(true);
        view.set_row_height(40.);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.filtered.borrow().len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let mut view = self.view.as_ref().unwrap().dequeue::<ContactPickerRow>(CONTACT_ROW);
        if let Some(delegate) = &mut view.delegate {
            let contacts = self.filtered.borrow();
            if let Some(channel) = contacts.get(row) {
                delegate.title.set_text(&channel.title);
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        if let Some(row) = row {
            let contacts = self.filtered.borrow();
            if let Some(channel) = contacts.get(row) {
                let channel_id = channel.id.clone();
                if let Some(tx) = self.backend_state.command_tx.lock().unwrap().as_ref() {
                    let _ = tx.unbounded_send(BackendCommand::LoadMessages(channel_id));
                }
            }
        }
    }
}

// — Window delegate —

pub struct ContactPickerDelegate {
    content: View,
    search_field: ContactSearchField,
    list: ListView<ContactListDelegate>,
    no_results: Label,
}

impl ContactPickerDelegate {
    fn new(backend_state: Arc<BackendState>) -> Self {
        Self {
            content: View::new(),
            search_field: new_contact_search_field(),
            list: ListView::with(ContactListDelegate::new(backend_state)),
            no_results: Label::new(),
        }
    }
}

impl WindowDelegate for ContactPickerDelegate {
    const NAME: &'static str = "ContactPickerWindow";

    fn did_load(&mut self, window: Window) {
        window.set_title("New Chat");
        window.set_titlebar_appears_transparent(true);

        self.search_field.set_placeholder_text("Search Contacts");

        self.no_results.set_text("No Results");
        self.no_results.set_text_color(Color::SystemGray);
        self.no_results.set_font(&Font::system(14.));
        self.no_results.set_hidden(true);

        self.content.add_subview(&self.search_field);
        self.content.add_subview(&self.list);
        self.content.add_subview(&self.no_results);

        LayoutConstraint::activate(&[
            self.search_field.top.constraint_equal_to(&self.content.top).offset(8.),
            self.search_field.leading.constraint_equal_to(&self.content.leading).offset(8.),
            self.search_field.trailing.constraint_equal_to(&self.content.trailing).offset(-8.),
            self.search_field.height.constraint_equal_to_constant(24.),
            self.list.top.constraint_equal_to(&self.search_field.bottom).offset(4.),
            self.list.bottom.constraint_equal_to(&self.content.bottom),
            self.list.leading.constraint_equal_to(&self.content.leading),
            self.list.trailing.constraint_equal_to(&self.content.trailing),
            self.no_results.center_x.constraint_equal_to(&self.content.center_x),
            self.no_results.center_y.constraint_equal_to(&self.content.center_y),
        ]);

        window.set_content_view(&self.content);
    }
}

// — Public window wrapper —

pub struct ContactPickerWindow(pub Option<Window<ContactPickerDelegate>>);

impl ContactPickerWindow {
    pub fn show_with_contacts(&self, contacts: Vec<CoreChannel>) {
        if let Some(ref w) = self.0 {
            if let Some(ref delegate) = w.delegate {
                if let Some(ref list_delegate) = delegate.list.delegate {
                    list_delegate.set_contacts(contacts);
                }
                delegate.no_results.set_hidden(true);
            }
            w.show();
            unsafe {
                use objc::{msg_send, sel, sel_impl};
                let _: () = msg_send![&*w.objc, center];
            }
        }
    }

    /// Silently refresh the contact list (no window show/focus).
    pub fn update_contacts(&self, contacts: Vec<CoreChannel>) {
        if let Some(ref w) = self.0 {
            if let Some(ref delegate) = w.delegate {
                if let Some(ref list_delegate) = delegate.list.delegate {
                    list_delegate.set_contacts(contacts);
                }
            }
        }
    }

    pub fn filter_contacts(&self, query: &str) {
        if let Some(ref w) = self.0 {
            if let Some(ref delegate) = w.delegate {
                if let Some(ref list_delegate) = delegate.list.delegate {
                    let empty = list_delegate.set_filter(query);
                    delegate.no_results.set_hidden(!empty);
                    delegate.list.set_hidden(empty);
                }
            }
        }
    }
}

impl ContactPickerWindow {
    pub fn new(backend_state: Arc<BackendState>) -> Self {
        let mut config = WindowConfig::default();
        config.set_styles(&[
            WindowStyle::Titled,
            WindowStyle::Closable,
            WindowStyle::Miniaturizable,
            WindowStyle::Resizable,
        ]);
        config.initial_dimensions = Rect::new(0.0, 0.0, 320.0, 480.0);
        Self(Some(Window::with(config, ContactPickerDelegate::new(backend_state))))
    }
}
