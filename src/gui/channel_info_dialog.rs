use gdk::prelude::{IsA, ObjectExt};
use glib::Object;
use gtk::glib;

use crate::backend::{Channel, Manager};

glib::wrapper! {
    pub struct ChannelInfoDialog(ObjectSubclass<imp::ChannelInfoDialog>)
        @extends libadwaita::MessageDialog, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelInfoDialog {
    pub fn new(channel: &Channel, manager: &Manager, parent: &impl IsA<gtk::Window>) -> Self {
        log::trace!("Initializing `ChannelInfoDialog`");
        let s = Object::builder::<Self>()
            .property("channel", channel)
            .property("manager", manager)
            .property("transient-for", parent)
            .build();
        s
    }

    pub fn channel(&self) -> Channel {
        self.property("channel")
    }
}

pub mod imp {
    use libadwaita::subclass::prelude::MessageDialogImpl;
    use std::cell::RefCell;

    use glib::{
        once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecObject, Value,
    };
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    use crate::backend::{Channel, Manager};
    use crate::gspawn;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_info_dialog.ui")]
    pub struct ChannelInfoDialog {
        channel: RefCell<Option<Channel>>,
        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl ChannelInfoDialog {
        #[template_callback]
        fn reset_session(&self) {
            let obj = self.obj();
            let channel = obj.channel();
            crate::trace!("Resetting session of channel {}", channel.title());
            gspawn!(async move { channel.send_session_reset().await });
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelInfoDialog {
        const NAME: &'static str = "FlChannelInfoDialog";
        type Type = super::ChannelInfoDialog;
        type ParentType = libadwaita::MessageDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelInfoDialog {
        fn constructed(&self) {
            self.parent_constructed();
        }

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
                    let man = value.get::<Option<Manager>>().expect(
                        "Property `manager` of `ChannelInfoDialog` has to be of type `Manager`",
                    );
                    self.manager.replace(man);
                }
                "channel" => {
                    let msg = value.get::<Option<Channel>>().expect(
                        "Property `channel` of `ChannelInfoDialog` has to be of type `Channel`",
                    );
                    self.channel.replace(msg);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WindowImpl for ChannelInfoDialog {}
    impl MessageDialogImpl for ChannelInfoDialog {}
    impl WidgetImpl for ChannelInfoDialog {}
    impl BoxImpl for ChannelInfoDialog {}
}
