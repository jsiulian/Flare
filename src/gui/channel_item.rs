use gdk::prelude::ObjectExt;
use glib::Object;
use gtk::glib;

use crate::backend::Channel;

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

    pub fn channel(&self) -> Channel {
        self.property("channel")
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
    use gtk::{glib, Label};
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};

    use crate::backend::message::{DisplayMessage, DisplayMessageExt, MessageExt};
    use crate::{
        backend::{Channel, Manager},
        gui::utility::Utility,
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_item.ui")]
    pub struct ChannelItem {
        #[template_child]
        label_last_message: TemplateChild<Label>,

        channel: RefCell<Option<Channel>>,

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
                gettextrs::gettext("is typing")
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
    impl BoxImpl for ChannelItem {}
}
