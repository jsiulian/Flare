use gdk_pixbuf::glib::Object;

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
}

pub mod imp {
    use std::cell::Cell;
    use std::cell::RefCell;

    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::subclass::Signal;
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

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/message_item.ui")]
    pub struct MessageItem {
        message: RefCell<Option<Message>>,
        expanded: Cell<bool>,
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
        fn handle_reply(&self) {
            let obj = self.instance();
            let msg = obj.property::<Message>("message");
            crate::trace!(
                "Replying to message {}",
                msg.property::<Option<String>>("body")
                    .unwrap_or("".to_string())
            );
            obj.emit_by_name::<()>("reply", &[&msg]);
        }
    }

    impl ObjectImpl for MessageItem {
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
                        "expanded",
                        "expanded",
                        "expanded",
                        false,
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
                        "has-quote",
                        "has-quote",
                        "has-quote",
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
                "expanded" => self.expanded.get().to_value(),
                "show-name" => self.show_name.get().to_value(),
                "has-quote" => self
                    .message
                    .borrow()
                    .as_ref()
                    .map(|m| m.property::<Option<Message>>("quote").is_some())
                    .unwrap_or_default()
                    .to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
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
                    self.message.replace(msg);
                }
                "expanded" => {
                    let exp = value
                        .get::<bool>()
                        .expect("Property `expanded` of `MessageItem` has to be of type `bool`");
                    self.expanded.replace(exp);
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
