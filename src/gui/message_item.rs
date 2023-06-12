use gdk::glib::BindingFlags;
use gio::{subclass::prelude::ObjectSubclassIsExt, SimpleAction, SimpleActionGroup};
use glib::{clone, Object};
use gtk::{gio, glib};
use gtk::{
    prelude::*,
    traits::{PopoverExt, WidgetExt},
};

use crate::backend::message::{MessageExt, TextMessage};
use crate::backend::timeline::timeline_item::TimelineItemExt;
use crate::backend::Manager;

glib::wrapper! {
    pub struct MessageItem(ObjectSubclass<imp::MessageItem>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl MessageItem {
    pub fn new(message: &TextMessage) -> Self {
        log::trace!("Initializing `MessageItem`");
        let s = Object::builder::<Self>()
            .property("message", message)
            .build();
        s.init_label_selectable();
        s.setup_showheader();
        s.setup_from_self();
        s
    }

    pub fn message(&self) -> TextMessage {
        self.property("message")
    }

    pub fn manager(&self) -> Manager {
        self.message().property("manager")
    }

    fn setup_actions(&self) {
        let action_reply = SimpleAction::new("reply", None);
        action_reply.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_reply();
        }));

        let action_react = SimpleAction::new("react", None);
        action_react.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_react_open();
        }));

        let action_delete = SimpleAction::new("delete", None);
        action_delete.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_delete();
        }));

        let action_copy = SimpleAction::new("copy", None);
        action_copy.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_copy();
        }));

        let actions = SimpleActionGroup::new();
        self.insert_action_group("msg", Some(&actions));
        actions.add_action(&action_reply);
        actions.add_action(&action_react);
        actions.add_action(&action_delete);
        actions.add_action(&action_copy);

        self.bind_property("message", &action_delete, "enabled")
            .transform_to(|_, msg: Option<TextMessage>| msg.map(|m| m.sender().is_self()))
            .flags(BindingFlags::SYNC_CREATE)
            .build();
    }

    fn setup_click(&self) {
        let gesture = gtk::GestureClick::new();
        gesture.connect_released(clone!(@weak self as s => move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            s.open_popup();
        }));
        self.imp().message_box.add_controller(gesture);
    }

    fn setup_showheader(&self) {
        self.message().connect_notify_local(
            Some("show-header"),
            clone!(@weak self as s => move |_, _| s.set_show_header()),
        );
        self.connect_notify_local(
            Some("force-show-header"),
            clone!(@weak self as s => move |_, _| s.set_show_header()),
        );
        self.set_show_header();
    }

    fn setup_from_self(&self) {
        if self.message().sender().is_self() {
            self.add_css_class("from-self");
            self.set_halign(gtk::Align::End);
        } else {
            self.set_halign(gtk::Align::Start);
        }
    }

    fn init_label_selectable(&self) {
        let manager = self.manager();
        let settings = manager.settings();
        let label = &self.imp().label_message;

        settings
            .bind("messages-selectable", &**label, "selectable")
            .build();
    }

    pub fn open_popup(&self) {
        self.imp().msg_menu.popup();
    }

    /// Set whether this item should show its header.
    pub fn set_show_header(&self) {
        let visible = self.message().show_header() || self.property("force-show-header");

        self.imp().header.set_visible(visible);

        if visible && !self.has_css_class("has-header") {
            self.add_css_class("has-header");
        } else if !visible && self.has_css_class("has-header") {
            self.remove_css_class("has-header");
        }
    }
}

pub mod imp {
    use lazy_static::lazy_static;
    use regex::Regex;
    use std::cell::{Cell, RefCell};

    use glib::{
        clone,
        once_cell::sync::Lazy,
        subclass::{InitializingObject, Signal},
        ParamSpec, ParamSpecBoolean, ParamSpecObject, Value,
    };
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    use crate::{
        backend::{message::TextMessage, Manager},
        gspawn,
        gui::{attachment::Attachment, error_dialog::ErrorDialog, utility::Utility},
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/message_item.ui")]
    pub struct MessageItem {
        #[template_child]
        pub(super) header: TemplateChild<gtk::Box>,
        #[template_child]
        emoji_chooser: TemplateChild<gtk::EmojiChooser>,
        #[template_child]
        pub(super) msg_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        box_attachments: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) label_message: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) message_box: TemplateChild<gtk::Box>,

        message: RefCell<Option<TextMessage>>,

        manager: RefCell<Option<Manager>>,

        force_show_header: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageItem {
        const NAME: &'static str = "FlMessageItem";
        type Type = super::MessageItem;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl MessageItem {
        #[template_callback(function)]
        fn markup_urls(s: Option<String>) -> Option<String> {
            lazy_static! {
                static ref RE: Regex = Regex::new(r#"(?P<l>[a-z]*://[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=;]*))"#).unwrap();
            }
            let s = s.map(|s| glib::markup_escape_text(&s));
            s.map(|s| RE.replace_all(&s, r#"<a href="$l">$l</a>"#).to_string())
        }

        #[template_callback]
        pub(super) fn handle_reply(&self) {
            let obj = self.obj();
            let msg = obj.message();
            // TODO: Log message
            crate::trace!("Replying to a message",);
            obj.emit_by_name::<()>("reply", &[&msg]);
        }

        #[template_callback]
        pub(super) fn handle_delete(&self) {
            let obj = self.obj();
            let msg = obj.message();

            gspawn!(async move { msg.delete().await });
        }

        #[template_callback]
        pub(super) fn handle_react_open(&self) {
            crate::trace!("Opening emoji dropdown",);
            self.emoji_chooser.popup();
        }

        #[template_callback]
        fn handle_react(&self, mut emoji: String) {
            let obj = self.obj();
            let msg = obj.message();

            // Remove the last three bytes. For some reason, the GTK picker adds two "variable
            // selector"s (e.g. bytes "239, 184, 143" ) to the the end of the string, which Signal
            // does not like. Remove one instance.
            // XXX: Wait until the next version of GTK is released (https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/5898).
            emoji.truncate(emoji.len() - 3);

            crate::trace!(
                "Reacting to message {} with {}",
                msg.body().unwrap_or_default(),
                emoji
            );

            let obj = self.obj();
            gspawn!(clone!(@strong msg, @strong obj => async move {
                log::trace!("Sending message");
                if let Err(e) = msg.send_reaction(emoji).await {
                    let root = obj
                        .root()
                        .expect("`MessageItem` to have a root")
                        .dynamic_cast::<crate::gui::Window>()
                        .expect("Root of `ChannelMessages` to be a `Window`.");
                    let dialog = ErrorDialog::new(e, &root);
                    dialog.present();
                }
                obj.notify("has-reaction");
            }));
        }

        #[template_callback]
        pub(super) fn handle_copy(&self) {
            let obj = self.obj();
            let display = gdk::Display::default().expect("there should be a display");
            let clipboard = display.clipboard();
            let msg = obj.message();
            // TODO: Log message
            crate::trace!("Copying message to clipboard",);
            if let Some(text) = msg.body() {
                clipboard.set_text(&text)
            }
        }
    }

    impl ObjectImpl for MessageItem {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_actions();
            self.obj().setup_click();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<TextMessage>("message").build(),
                    ParamSpecBoolean::builder("force-show-header")
                        .default_value(true)
                        .build(),
                    ParamSpecBoolean::builder("has-reaction")
                        .read_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "message" => self.message.borrow().as_ref().to_value(),
                "has-reaction" => self
                    .message
                    .borrow()
                    .as_ref()
                    .map(|m| !m.reactions().is_empty())
                    .unwrap_or_default()
                    .to_value(),
                "force-show-header" => self.force_show_header.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            let instance = self.obj();
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `MessageItem` has to be of type `Manager`");
                    self.manager.replace(man);
                }
                "message" => {
                    let msg = value
                        .get::<Option<TextMessage>>()
                        .expect("Property `message` of `MessageItem` has to be of type `Message`");
                    if let Some(msg) = &msg {
                        msg.connect_notify_local(
                            Some("reactions"),
                            clone!(@weak instance as obj => move |_, _| {
                                log::trace!("MessageItem got reaction, updating `has-reaction`");
                                obj.notify("has-reaction");
                            }),
                        );
                        // Clear attachments
                        while let Some(at) = self.box_attachments.first_child() {
                            self.box_attachments.remove(&at);
                        }
                        // Set attachments
                        for att in msg.attachments() {
                            log::trace!("MessageItem got Attachment, adding to `box_attachments`");
                            let att_widget = Attachment::new(&att);
                            self.box_attachments.append(&att_widget);
                        }
                    }
                    instance.notify("has-reaction");
                    self.message.replace(msg);
                }
                "force-show-header" => {
                    let b = value.get::<bool>().expect(
                        "Property `force-show-header` of `MessageItem` has to be of type `bool`",
                    );
                    self.force_show_header.replace(b);
                }
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder("reply")
                    .param_types([TextMessage::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for MessageItem {}
    impl BoxImpl for MessageItem {}
}
