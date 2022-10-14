use gio::{subclass::prelude::ObjectSubclassIsExt, SimpleAction, SimpleActionGroup};
use glib::{clone, Object};
use gtk::{
    prelude::*,
    traits::{PopoverExt, WidgetExt},
};

use crate::backend::message::TextMessage;

gtk::glib::wrapper! {
    pub struct MessageItem(ObjectSubclass<imp::MessageItem>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl MessageItem {
    pub fn new(message: &TextMessage) -> Self {
        log::trace!("Initializing `MessageItem`");
        Object::new(&[("message", message)]).expect("Failed to create `MessageItem`")
    }

    pub fn message(&self) -> TextMessage {
        self.property("message")
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

        let actions = SimpleActionGroup::new();
        self.insert_action_group("msg", Some(&actions));
        actions.add_action(&action_reply);
        actions.add_action(&action_react);
    }

    pub fn open_popup(&self) {
        self.imp().msg_menu.popup();
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
        ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecObject, Value,
    };
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
        emoji_chooser: TemplateChild<gtk::EmojiChooser>,
        #[template_child]
        pub(super) msg_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        box_attachments: TemplateChild<gtk::Box>,

        message: RefCell<Option<TextMessage>>,
        show_name: Cell<bool>,

        manager: RefCell<Option<Manager>>,
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
                static ref RE: Regex = Regex::new(r#"(?P<l>[a-z]*://[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*))"#).unwrap();
            }
            let s = s.map(|s| {
                s.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            });
            s.map(|s| RE.replace_all(&s, r#"<a href="$l">$l</a>"#).to_string())
        }

        #[template_callback]
        pub(super) fn handle_reply(&self) {
            let obj = self.instance();
            let msg = obj.message();
            // TODO: Log message
            crate::trace!("Replying to a message",);
            obj.emit_by_name::<()>("reply", &[&msg]);
        }

        #[template_callback]
        pub(super) fn handle_react_open(&self) {
            crate::trace!("Opening emoji dropdown",);
            self.emoji_chooser.popup();
        }

        #[template_callback]
        fn handle_react(&self, emoji: String) {
            let obj = self.instance();
            let msg = obj.message();
            crate::trace!(
                "Reacting to message {} with {} (len: {})",
                msg.body().unwrap_or_else(|| "".to_string()),
                emoji,
                emoji.chars().count()
            );
            let obj = self.instance();
            gspawn!(clone!(@strong msg, @strong obj => async move {
                log::trace!("Sending message");
                if let Err(e) = msg.send_reaction(&emoji.chars().next().unwrap_or_default().to_string()).await {
                    let root = obj
                        .root()
                        .expect("`MessageItem` to have a root")
                        .dynamic_cast::<crate::gui::Window>()
                        .expect("Root of `ChannelMessages` to be a `Window`.");
                    let dialog = ErrorDialog::new(e, &root);
                    dialog.show();
                }
                obj.notify("has-reaction");
            }));
        }
    }

    impl ObjectImpl for MessageItem {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.setup_actions();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "message",
                        "message",
                        "message",
                        TextMessage::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        "show-name",
                        "show-name",
                        "show-name",
                        true,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        "has-reaction",
                        "has-reaction",
                        "has-reaction",
                        false,
                        ParamFlags::READABLE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "message" => self.message.borrow().as_ref().to_value(),
                "show-name" => self.show_name.get().to_value(),
                "has-reaction" => self
                    .message
                    .borrow()
                    .as_ref()
                    .map(|m| !m.reactions().is_empty())
                    .unwrap_or_default()
                    .to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
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
                            clone!(@strong obj => move |_, _| {
                                log::trace!("MessageItem got reaction, updating `has-reaction`");
                                obj.notify("has-reaction");
                            }),
                        );
                        for att in msg.attachments() {
                            log::trace!("MessageItem got Attachment, adding to `box_attachments`");
                            let att_widget = Attachment::new(&att);
                            self.box_attachments.append(&att_widget);
                        }
                    }
                    obj.notify("has-reaction");
                    self.message.replace(msg);
                }
                "show-name" => {
                    let show = value
                        .get::<bool>()
                        .expect("Property `show-name` of `MessageItem` has to be of type `bool`");
                    self.show_name.replace(show);
                }
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder(
                    "reply",
                    &[TextMessage::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for MessageItem {}
    impl BoxImpl for MessageItem {}
}
