use crate::prelude::*;

glib::wrapper! {
    /// A channel displayed in the sidebar.
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
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::{CompositeTemplate, Label};

    use crate::backend::message::{DisplayMessage, DisplayMessageExt, MessageExt};
    use crate::backend::{Channel, Manager};

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::ChannelItem)]
    #[template(resource = "/ui/channel_item.ui")]
    pub struct ChannelItem {
        #[template_child]
        label_last_message: TemplateChild<Label>,

        #[property(get, set, type = Channel)]
        channel: RefCell<Option<Channel>>,

        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl ChannelItem {
        #[template_callback]
        fn format_last_message(
            &self,
            message: Option<DisplayMessage>,
            is_typing: bool,
            draft: String,
        ) -> String {
            if is_typing {
                self.label_last_message.add_css_class("accent");
                self.label_last_message.remove_css_class("dim-label");
                self.obj().channel().typing_label()
            } else if !draft.is_empty() {
                format!(
                    "<span font-weight='500'>{}:</span> {}",
                    gettextrs::gettext("Draft"),
                    glib::markup_escape_text(draft.as_str())
                )
            } else if let Some(msg) = message {
                self.label_last_message.add_css_class("dim-label");
                self.label_last_message.remove_css_class("accent");

                let is_group = self.obj().channel().group().is_some();
                if is_group {
                    format!(
                        "<span font-weight='500'>{}:</span> {}",
                        glib::markup_escape_text(&msg.sender().title()),
                        glib::markup_escape_text(&msg.textual_description().unwrap_or_default())
                    )
                } else {
                    format!(
                        "{}",
                        glib::markup_escape_text(&msg.textual_description().unwrap_or_default())
                    )
                }
            } else {
                String::new()
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelItem {
        const NAME: &'static str = "FlChannelItem";
        type Type = super::ChannelItem;
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

    #[glib::derived_properties]
    impl ObjectImpl for ChannelItem {}

    impl WidgetImpl for ChannelItem {}
    impl BoxImpl for ChannelItem {}
}
