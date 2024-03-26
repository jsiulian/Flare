use crate::prelude::*;

glib::wrapper! {
    /// A compact representation for [ChannelItem], used in [ChannelInfoDialog](crate::gui::channel_info_dialog::ChannelInfoDialog).
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
}

impl Default for ChannelItemCompact {
    fn default() -> Self {
        Self::new()
    }
}

pub mod imp {
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;

    use crate::backend::Channel;

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::ChannelItemCompact)]
    #[template(resource = "/ui/channel_item_compact.ui")]
    pub struct ChannelItemCompact {
        #[template_child]
        label_name: TemplateChild<gtk::Label>,

        #[property(get, set = Self::set_channel)]
        channel: RefCell<Option<Channel>>,
    }

    impl ChannelItemCompact {
        fn set_channel(&self, chan: Option<Channel>) {
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

    #[glib::derived_properties]
    impl ObjectImpl for ChannelItemCompact {}

    impl WidgetImpl for ChannelItemCompact {}
    impl BoxImpl for ChannelItemCompact {}
}
