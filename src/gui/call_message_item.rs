use glib::Object;
use gtk::glib;
use gtk::prelude::*;

use crate::backend::message::CallMessage;

gtk::glib::wrapper! {
    pub struct CallMessageItem(ObjectSubclass<imp::CallMessageItem>)
        @extends gtk::Box,gtk::Widget;
}

impl CallMessageItem {
    pub fn new(message: &CallMessage) -> Self {
        log::trace!("Initializing `CallMessageItem`");
        Object::builder::<Self>()
            .property("message", message)
            .build()
    }

    pub fn message(&self) -> CallMessage {
        self.property("message")
    }
}

pub mod imp {
    use std::cell::RefCell;

    use glib::{
        once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecObject, Value,
    };
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    use crate::{
        backend::{message::CallMessage, Manager},
        gui::utility::Utility,
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/call_message_item.ui")]
    pub struct CallMessageItem {
        message: RefCell<Option<CallMessage>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CallMessageItem {
        const NAME: &'static str = "FlCallMessageItem";
        type Type = super::CallMessageItem;
        type ParentType = gtk::Box;
        
        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CallMessageItem {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<CallMessage>("message").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "message" => self.message.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value.get::<Option<Manager>>().expect(
                        "Property `manager` of `CallMessageItem` has to be of type `Manager`",
                    );
                    self.manager.replace(man);
                }
                "message" => {
                    let msg = value.get::<Option<CallMessage>>().expect(
                        "Property `message` of `CallMessageItem` has to be of type `Message`",
                    );
                    self.message.replace(msg);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for CallMessageItem {}
    impl BoxImpl for CallMessageItem {}
}
