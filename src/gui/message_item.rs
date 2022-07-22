use gdk_pixbuf::{
    glib::{self, clone, Object},
    prelude::ActionMapExt,
};
use gio::{subclass::prelude::ObjectSubclassIsExt, SimpleAction, SimpleActionGroup};
use gtk::traits::{PopoverExt, WidgetExt};

use crate::backend::Message;

gtk::glib::wrapper! {
    pub struct MessageItem(ObjectSubclass<imp::MessageItem>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl MessageItem {
    pub fn new(message: &Message) -> Self {
        log::trace!("Initializing `MessageItem`");
        Object::new(&[("message", message)]).expect("Failed to create `MessageItem`")
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
    use std::cell::Cell;
    use std::cell::RefCell;

    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::subclass::Signal;
    use gdk_pixbuf::glib::MainContext;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecBoolean;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::Value;
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;

    use crate::backend::Manager;
    use crate::backend::Message;
    use crate::gui::attachment::Attachment;
    use crate::gui::error_dialog::ErrorDialog;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/message_item.ui")]
    pub struct MessageItem {
        #[template_child]
        emoji_chooser: TemplateChild<gtk::EmojiChooser>,
        #[template_child]
        pub(super) msg_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        box_attachments: TemplateChild<gtk::Box>,

        message: RefCell<Option<Message>>,
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
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl MessageItem {
        #[template_callback]
        pub(super) fn handle_reply(&self) {
            let obj = self.instance();
            let msg = obj.property::<Message>("message");
            crate::trace!(
                "Replying to message {}",
                msg.property::<Option<String>>("body")
                    .unwrap_or_else(|| "".to_string())
            );
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
            let msg = obj.property::<Message>("message");
            crate::trace!(
                "Reacting to message {} with {} (len: {})",
                msg.property::<Option<String>>("body")
                    .unwrap_or_else(|| "".to_string()),
                emoji,
                emoji.chars().count()
            );
            let main_context = MainContext::default();
            let obj = self.instance();
            main_context.spawn_local(clone!(@strong msg, @strong obj => async move {
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
            }));
        }

        #[template_callback(function)]
        fn is_some(opt: Option<glib::Object>) -> bool {
            opt.is_some()
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
                        Message::static_type(),
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
                    .map(|m| !m.property::<String>("reactions").is_empty())
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
                        .get::<Option<Message>>()
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
                    &[Message::static_type().into()],
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
