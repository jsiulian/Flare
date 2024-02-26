use gio::{subclass::prelude::ObjectSubclassIsExt, SimpleAction, SimpleActionGroup};
use glib::{clone, Object};
use gtk::prelude::*;
use gtk::{gio, glib};
use regex::Regex;

use crate::backend::message::{MessageExt, TextMessage};
use crate::backend::timeline::timeline_item::TimelineItemExt;
use crate::backend::Manager;
use crate::gui::attachment::Attachment;
use crate::gui::components::*;

glib::wrapper! {
    pub struct MessageItem(ObjectSubclass<imp::MessageItem>)
        @extends adw::Bin, gtk::Widget, ContextMenuBin,
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
        s.setup_showtimestamp();
        s.setup_from_group();
        s.setup_from_self();
        s.setup_quote();
        s.setup_emoji();
        s.setup_loaded();
        s.setup_text();
        s
    }

    pub fn message(&self) -> TextMessage {
        self.property("message")
    }

    pub fn manager(&self) -> Manager {
        self.message().property("manager")
    }
    pub fn get_popover(&self) -> gtk::PopoverMenu {
        self.imp().msg_menu.to_owned()
    }

    pub fn get_pressed_attachment(&self) -> Attachment {
        self.property("pressed-attachment")
    }

    fn setup_actions(&self) {
        let action_reply = SimpleAction::new("reply", None);
        action_reply.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_reply();
        }));

        let action_delete = SimpleAction::new("delete", None);
        action_delete.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_delete();
        }));

        let action_copy = SimpleAction::new("copy", None);
        action_copy.connect_activate(clone!(@weak self as s => move |_, _| {
            s.imp().handle_copy();
        }));

        let action_download = SimpleAction::new("download", None);
        action_download.connect_activate(clone!(@weak self as s => move |_, _| {
            s.get_pressed_attachment().imp().download();
        }));

        let action_open = SimpleAction::new("open", None);
        action_open.connect_activate(clone!(@weak self as s => move |_, _| {
            s.get_pressed_attachment().imp().open();
        }));

        let actions = SimpleActionGroup::new();
        self.insert_action_group("msg", Some(&actions));
        actions.add_action(&action_reply);
        actions.add_action(&action_delete);
        actions.add_action(&action_copy);
        actions.add_action(&action_download);
        actions.add_action(&action_open);

        self.bind_property("message", &action_delete, "enabled")
            .transform_to(|_, msg: Option<TextMessage>| msg.map(|m| m.sender().is_self()))
            .sync_create()
            .build();

        self.bind_property("pressed-attachment", &action_download, "enabled")
            .transform_to(|_, att: Option<Attachment>| Some(att.is_some()))
            .sync_create()
            .build();

        action_download
            .bind_property("enabled", &action_open, "enabled")
            .sync_create()
            .build();
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

    fn setup_showtimestamp(&self) {
        self.message().connect_notify_local(
            Some("show-timestamp"),
            clone!(@weak self as s => move |_, _| s.set_show_timestamp()),
        );
        self.connect_notify_local(
            Some("force-show-timestamp"),
            clone!(@weak self as s => move |_, _| s.set_show_timestamp()),
        );
        self.set_show_timestamp();
    }

    fn setup_quote(&self) {
        if self.message().quote().is_some() {
            self.imp().message_bubble.add_css_class("has-quote");
        }
    }

    fn setup_text(&self) {
        if self.message().body().is_some() {
            self.imp().message_bubble.add_css_class("has-text");
            self.imp().timestamp_img.set_visible(false);
        } else {
            self.imp().timestamp.set_visible(false);
        }
    }

    fn setup_from_self(&self) {
        if self.message().sender().is_self() {
            self.add_css_class("from-self");
            self.message().set_show_header(false);
            self.set_halign(gtk::Align::End);
            self.imp().reactions.set_halign(gtk::Align::Start);
            self.imp().message_bubble.set_halign(gtk::Align::End);
            self.imp().reaction_grid.set_halign(gtk::Align::End);
        } else {
            self.set_halign(gtk::Align::Start);
            self.imp().reactions.set_halign(gtk::Align::End);
            self.imp().message_bubble.set_halign(gtk::Align::Start);
            self.imp().reaction_grid.set_halign(gtk::Align::Start);
        }
    }

    fn setup_from_group(&self) {
        if self.message().channel().group().is_none() {
            self.message().set_show_header(false);
            self.imp().avatar.set_visible(false);
        }
    }

    fn setup_emoji(&self) {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"^[[\p{Emoji}--\p{Ascii}]\u{fe0f}\u{200d} ]+$").unwrap();
        }
        if self.message().body().is_some() && RE.is_match(self.message().body().unwrap().as_str()) {
            self.imp().message_bubble.add_css_class("emoji");
            self.message().set_show_header(false);
        }
    }

    fn init_label_selectable(&self) {
        let manager = self.manager();
        let settings = manager.settings();
        let label = &self.imp().label_message.imp().label;

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

    /// Set whether this item should show its timestamp.
    pub fn set_show_timestamp(&self) {
        let visible = self.message().show_timestamp() || self.property("force-show-timestamp");
        self.imp().timestamp.set_visible(visible);
        self.imp().timestamp_img.set_visible(visible);
    }

    fn setup_loaded(&self) {
        self.connect_notify_local(
            Some("shows-media-loading"),
            clone!(@weak self as s => move |_, _| {
                s.imp().box_attachments.remove_css_class("not-loaded");
            }),
        );
    }
}

pub mod imp {
    use lazy_static::lazy_static;
    use regex::Regex;
    use std::cell::{Cell, RefCell};

    use glib::{
        clone,
        subclass::{InitializingObject, Signal},
        ParamSpec, ParamSpecBoolean, ParamSpecObject, Value,
    };
    use gtk::{glib, EmojiChooser};
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};
    use once_cell::sync::Lazy;

    use crate::{
        backend::{message::TextMessage, Manager},
        gspawn,
        gui::{
            attachment::{backend_to_gui, Attachment},
            components::*,
            error_dialog::ErrorDialog,
            utility::Utility,
        },
    };
    use adw::subclass::prelude::BinImpl;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/message_item.ui")]
    pub struct MessageItem {
        #[template_child]
        pub(super) avatar: TemplateChild<adw::Avatar>,
        #[template_child]
        pub(super) header: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) reactions: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) msg_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub(super) box_attachments: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) media_overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        media_group: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) timestamp_img: TemplateChild<MessageIndicators>,
        #[template_child]
        download_btn: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) label_message: TemplateChild<MessageLabel>,
        #[template_child]
        pub reaction_grid: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) message_bubble: TemplateChild<gtk::Grid>,
        #[template_child]
        pub(super) timestamp: TemplateChild<MessageIndicators>,

        message: RefCell<Option<TextMessage>>,
        manager: RefCell<Option<Manager>>,
        pressed_attachment: RefCell<Option<Attachment>>,
        force_show_header: Cell<bool>,
        force_show_timestamp: Cell<bool>,
        has_attachment: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageItem {
        const NAME: &'static str = "FlMessageItem";
        type Type = super::MessageItem;
        type ParentType = ContextMenuBin;

        fn class_init(klass: &mut Self::Class) {
            EmojiPicker::ensure_type();
            MessageIndicators::ensure_type();
            MessageLabel::ensure_type();
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
                static ref RE: Regex = Regex::new(r"(?P<l>[a-z]*://[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=;]*))").unwrap();
            }
            let s = s.map(|s| glib::markup_escape_text(&s));
            s.map(|s| RE.replace_all(&s, r#"<a href="$l">$l</a>"#).to_string())
        }

        #[template_callback]
        pub fn open_emoji_picker(&self) {
            let obj = self.obj();

            let emoji_chooser = EmojiChooser::new();
            let (_, rectangle) = self.msg_menu.pointing_to();
            emoji_chooser.set_pointing_to(Some(&rectangle));

            emoji_chooser.connect_emoji_picked(clone!(@weak obj => move |_, emoji| {
                obj.imp().handle_react(emoji.to_owned());
            }));
            emoji_chooser.connect_local(
                "closed",
                false,
                clone!(@weak obj => @default-return None, move |values| {
                    let chooser = values[0].get::<EmojiChooser>().expect("Closed of EmojiChooser return value to be EmojiChooser");
                    chooser.unparent();
                    None
                }),
            );

            self.message_bubble.attach(&emoji_chooser, 0, 2, 1, 1);

            self.msg_menu.popdown();
            emoji_chooser.popup();
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

        // Signal uses the old unicode for the heart emoji, which is recognized as a black heart by gtk. This function converts it to the standard red heart
        #[template_callback(function)]
        pub(super) fn fix_emoji(emoji: Option<String>) -> Option<String> {
            if emoji == Some(String::from("❤")) {
                Some(String::from("❤\u{fe0f}"))
            } else {
                emoji
            }
        }

        #[template_callback]
        pub(super) fn handle_react(&self, mut emoji: String) {
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
            obj.imp().msg_menu.popdown();
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

        #[template_callback]
        pub(super) fn load_media(&self) {
            let obj = self.obj();
            let msg = obj.message();
            let btn = &obj.imp().download_btn;
            btn.set_can_target(false);
            let spinner = gtk::Spinner::new();
            spinner.start();
            btn.set_child(Some(&spinner));
            for att in msg.attachments() {
                gspawn!(async move { att.load().await });
            }
        }
    }

    impl ObjectImpl for MessageItem {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_actions();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<TextMessage>("message").build(),
                    ParamSpecObject::builder::<Attachment>("pressed-attachment").build(),
                    ParamSpecBoolean::builder("force-show-header")
                        .default_value(true)
                        .build(),
                    ParamSpecBoolean::builder("force-show-timestamp")
                        .default_value(true)
                        .build(),
                    ParamSpecBoolean::builder("shows-media-loading")
                        .read_only()
                        .build(),
                    ParamSpecBoolean::builder("has-reaction")
                        .read_only()
                        .build(),
                    // XXX: Make read_only.
                    ParamSpecBoolean::builder("has-attachment")
                        .default_value(false)
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "message" => self.message.borrow().as_ref().to_value(),
                "has-attachment" => self.has_attachment.get().to_value(),
                "has-reaction" => self
                    .message
                    .borrow()
                    .as_ref()
                    .map(|m| !m.reactions().is_empty())
                    .unwrap_or_default()
                    .to_value(),
                "force-show-header" => self.force_show_header.get().to_value(),
                "force-show-timestamp" => self.force_show_timestamp.get().to_value(),
                "shows-media-loading" => {
                    let mut value = false;

                    if let Some(msg) = self.message.borrow().as_ref() {
                        for att in msg.attachments() {
                            value = value || !att.loaded()
                        }
                    }

                    value.to_value()
                }
                "pressed-attachment" => self.pressed_attachment.borrow().as_ref().to_value(),
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
                        let attachments = msg.attachments();
                        let mut container = &self.box_attachments;

                        // Set attachments
                        if !attachments.is_empty() {
                            instance.notify("shows-media-loading");
                            instance.set_property("has-attachment", true);
                            self.box_attachments
                                .parent()
                                .unwrap()
                                .add_css_class("has-attachment");

                            if attachments[0].is_image() || attachments[0].is_video() {
                                container = &self.media_group;
                                self.media_overlay.set_visible(true);
                            }

                            for att in attachments {
                                log::trace!(
                                    "MessageItem got Attachment, adding to `box_attachments`"
                                );

                                let att_widget = backend_to_gui(&att);

                                att.connect_notify_local(
                                    Some("loaded"),
                                    clone!(@weak instance as obj => move |_, _| {
                                        obj.notify("shows-media-loading")
                                    }),
                                );

                                att_widget.connect_local(
                                    "pressed",
                                    false,
                                    clone!(@weak instance as obj, @weak att_widget as att => @default-return None, move |_| {
                                        obj.set_property("pressed-attachment", Some(att));
                                        None
                                    })
                                );
                                container.append(&att_widget);
                            }
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
                "force-show-timestamp" => {
                    let b = value.get::<bool>().expect(
                        "Property `force-show-timestamp` of `MessageItem` has to be of type `bool`",
                    );
                    self.force_show_timestamp.replace(b);
                }
                "has-attachment" => {
                    let b = value.get::<bool>().expect(
                        "Property `has-attachment` of `MessageItem` has to be of type `bool`",
                    );
                    self.has_attachment.replace(b);
                }
                "pressed-attachment" => {
                    let attachment = value.get::<Option<Attachment>>().expect(
                        "Property `message` of `MessageItem` has to be of type `Attachment`",
                    );
                    self.pressed_attachment.replace(attachment);
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
    impl BinImpl for MessageItem {}
    impl ContextMenuBinImpl for MessageItem {
        fn menu_opened(&self) {
            let obj = self.obj();
            let popover = obj.get_popover();
            obj.set_popover(Some(popover));
        }
    }
}
