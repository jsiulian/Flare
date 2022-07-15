use gdk_pixbuf::prelude::ObjectExt;
use gio::subclass::prelude::ObjectSubclassIsExt;

use crate::{backend::Channel, gui::channel_item::ChannelItem};

gtk::glib::wrapper! {
    pub struct ChannelList(ObjectSubclass<imp::ChannelList>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelList {
    fn add_channel(&self, channel: Channel) {
        crate::trace!(
            "`ChannelList` got new `Channel`: {}",
            channel.property::<String>("title")
        );
        let widget = ChannelItem::new(&channel);
        self.imp().list.prepend(&widget);
    }
}

pub mod imp {
    use std::cell::RefCell;

    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::subclass::Signal;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::Value;
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;

    use crate::backend::Channel;
    use crate::backend::Manager;
    use crate::backend::Message;
    use crate::gui::channel_item::ChannelItem;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_list.ui")]
    pub struct ChannelList {
        #[template_child]
        pub(super) list: TemplateChild<gtk::ListBox>,

        manager: RefCell<Option<Manager>>,
        active_channel: RefCell<Option<Channel>>,
    }

    #[gtk::template_callbacks]
    impl ChannelList {
        #[template_callback]
        fn handle_row_activated(&self, row: gtk::ListBoxRow) {
            let channel = row
                .child()
                .expect("`ListBoxRow` to have a child")
                .dynamic_cast::<ChannelItem>()
                .expect("`ListBoxRow` to have a `ChannelItem` child")
                .property::<Channel>("channel");
            crate::trace!("Activated channel: {}", channel.property::<String>("title"));
            self.instance().set_property("active-channel", channel);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelList {
        const NAME: &'static str = "FlChannelList";
        type Type = super::ChannelList;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelList {
        fn constructed(&self, _obj: &Self::Type) {
            self.list.set_sort_func(|l1, l2| {
                let c1 = l1
                    .child()
                    .expect("`ListBoxRow` of `ChannelList` to have a child")
                    .property::<Channel>("channel");
                let c2 = l2
                    .child()
                    .expect("`ListBoxRow` of `ChannelList` to have a child")
                    .property::<Channel>("channel");

                let m1 = c1.property::<Option<Message>>("last-message");
                let m2 = c2.property::<Option<Message>>("last-message");

                if m1.is_some() && m2.is_none() {
                    return gtk::Ordering::Smaller;
                } else if m1.is_none() && m2.is_some() {
                    return gtk::Ordering::Larger;
                } else if let (Some(m1), Some(m2)) = (m1, m2) {
                    let s1 = m1.property::<u64>("sent");
                    let s2 = m2.property::<u64>("sent");
                    if s1 > s2 {
                        return gtk::Ordering::Smaller;
                    } else {
                        return gtk::Ordering::Larger;
                    }
                }

                if c1.property::<String>("title") < c2.property::<String>("title") {
                    gtk::Ordering::Smaller
                } else {
                    gtk::Ordering::Larger
                }
            });
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
                        "active-channel",
                        "active-channel",
                        "active-channel",
                        Channel::static_type(),
                        ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "active-channel" => self.active_channel.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `ChannelList` has to be of type `Manager`");

                    if let Some(man) = &man {
                        log::trace!(
                            "Connecting to the `channel` signal of `Manager` in `ChannelList`"
                        );
                        man.connect_local(
                            "channel",
                            false,
                            clone!(@strong obj => move |args| {
                                let channel = args[1]
                                    .get::<Channel>()
                                    .expect("Type of `channel` signal of `Manager` to be `Channel`");
                                obj.add_channel(channel);
                                None
                            }),
                        );
                    }
                    self.manager.replace(man);
                }
                "active-channel" => {
                    let chan = value.get::<Option<Channel>>().expect(
                        "Property `active-channel` of `ChannelList` has to be of type `Channel`",
                    );
                    obj.emit_by_name::<()>("active-channel-changed", &[&chan]);
                    self.active_channel.replace(chan);
                }
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| -> Vec<Signal> {
                vec![Signal::builder(
                    "active-channel-changed",
                    &[Channel::static_type().into()],
                    <()>::static_type().into(),
                )
                .build()]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for ChannelList {}
    impl BoxImpl for ChannelList {}
}
