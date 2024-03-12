use gdk::prelude::ObjectExt;
use glib::Object;
use gtk::glib;

use crate::backend::Channel;

glib::wrapper! {
    pub struct ChannelItemCompact(ObjectSubclass<imp::ChannelItemCompact>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelItemCompact {
    pub fn new() -> Self {
        log::trace!("Initializing `ChannelItemCompact`");
        Object::builder::<Self>().build()
    }

    pub fn channel(&self) -> Channel {
        self.property("channel")
    }
}

impl Default for ChannelItemCompact {
    fn default() -> Self {
        Self::new()
    }
}

pub mod imp {
    use std::cell::RefCell;

    use glib::{subclass::InitializingObject, ParamSpec, ParamSpecObject, Value};
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};
    use once_cell::sync::Lazy;

    use crate::{backend::Channel, gui::utility::Utility};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_item_compact.ui")]
    pub struct ChannelItemCompact {
        #[template_child]
        label_name: TemplateChild<gtk::Label>,

        channel: RefCell<Option<Channel>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelItemCompact {
        const NAME: &'static str = "FlChannelItemCompact";
        type Type = super::ChannelItemCompact;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ChannelItemCompact {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> =
                Lazy::new(|| vec![ParamSpecObject::builder::<Channel>("channel").build()]);
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "channel" => self.channel.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "channel" => {
                    let chan = value.get::<Option<Channel>>().expect(
                        "Property `channel` of `ChannelItemCompact` has to be of type `Channel`",
                    );

                    let (part1, part2) = chan.as_ref().map(|c| c.name_parts()).unwrap_or_default();
                    let (part1, part2) = (
                        part1.map(|p| glib::markup_escape_text(&p)),
                        glib::markup_escape_text(&part2),
                    );

                    let format = if let Some(part1) = part1 {
                        format!("{} <b>{}</b>", part1, part2)
                    } else {
                        format!("<b>{}</b>", part2)
                    };
                    self.label_name.set_markup(&format);

                    self.channel.replace(chan);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for ChannelItemCompact {}
    impl BoxImpl for ChannelItemCompact {}
}
