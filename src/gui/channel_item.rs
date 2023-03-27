use glib::Object;
use gtk::glib;

glib::wrapper! {
    pub struct ChannelItem(ObjectSubclass<imp::ChannelItem>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelItem {
    pub fn new() -> Self {
        log::trace!("Initializing `ChannelItem`");
        Object::builder::<Self>().build()
    }
}

impl Default for ChannelItem {
    fn default() -> Self {
        Self::new()
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
        backend::{Channel, Manager},
        gui::utility::Utility,
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_item.ui")]
    pub struct ChannelItem {
        channel: RefCell<Option<Channel>>,

        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl ChannelItem {
        #[template_callback(function)]
        pub(super) fn append_colon(s: Option<String>) -> String {
            format!("{}: ", s.unwrap_or_default())
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelItem {
        const NAME: &'static str = "FlChannelItem";
        type Type = super::ChannelItem;
        type ParentType = gtk::Grid;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelItem {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Channel>("channel").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "channel" => self.channel.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `ChannelItem` has to be of type `Manager`");
                    self.manager.replace(man);
                }
                "channel" => {
                    let chan = value
                        .get::<Option<Channel>>()
                        .expect("Property `channel` of `ChannelItem` has to be of type `Channel`");
                    self.channel.replace(chan);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for ChannelItem {}
    impl GridImpl for ChannelItem {}
}
