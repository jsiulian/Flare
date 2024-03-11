use gdk::{glib::subclass::types::ObjectSubclassIsExt, prelude::ObjectExt};
use glib::Object;
use gtk::glib;

use crate::backend::{Channel, Manager};

glib::wrapper! {
    pub struct ChannelInfoDialog(ObjectSubclass<imp::ChannelInfoDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl ChannelInfoDialog {
    pub fn new(channel: &Channel, manager: &Manager) -> Self {
        log::trace!("Initializing `ChannelInfoDialog`");
        let s = Object::builder::<Self>()
            .property("channel", channel)
            .property("manager", manager)
            .build();
        s.imp().setup();
        s
    }

    pub fn channel(&self) -> Channel {
        self.property("channel")
    }
}

pub mod imp {
    use adw::prelude::ActionRowExt;
    use adw::subclass::dialog::AdwDialogImpl;
    use std::cell::RefCell;

    use glib::{subclass::InitializingObject, ParamSpec, ParamSpecObject, Value};
    use gtk::glib;
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};
    use once_cell::sync::Lazy;

    use crate::backend::{Channel, Manager};
    use crate::gspawn;
    use crate::gui::utility::Utility;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/channel_info_dialog.ui")]
    pub struct ChannelInfoDialog {
        #[template_child]
        avatar: TemplateChild<adw::Avatar>,
        #[template_child]
        row_phone: TemplateChild<adw::ActionRow>,
        #[template_child]
        row_disappearing: TemplateChild<adw::ActionRow>,
        #[template_child]
        row_description: TemplateChild<adw::ActionRow>,
        #[template_child]
        button_reset_session: TemplateChild<gtk::Button>,

        channel: RefCell<Option<Channel>>,
        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl ChannelInfoDialog {
        // For some reason, expressions lead to a crash in the UI. Do it manually.
        pub(super) fn setup(&self) {
            let binding = self.channel.borrow();
            let channel = binding.as_ref().expect("channel to be set at setup");
            self.avatar.set_text(Some(&channel.title()));
            self.avatar.set_show_initials(!channel.is_self());
            self.avatar.set_custom_image(channel.avatar().as_ref());

            let phone = channel.phone_number();
            self.row_phone.set_visible(phone.is_some());
            self.row_phone.set_subtitle(&phone.unwrap_or_default());

            self.row_disappearing
                .set_subtitle(&Self::format_disappearing_messages_timer(
                    channel.disappearing_messages_timer(),
                ));

            let description = channel.description();
            self.row_description.set_visible(description.is_some());
            self.row_description
                .set_subtitle(&description.unwrap_or_default());

            self.button_reset_session.set_visible(channel.is_contact());
        }

        #[template_callback]
        fn reset_session(&self) {
            let obj = self.obj();
            let channel = obj.channel();
            crate::trace!("Resetting session of channel {}", channel.title());
            gspawn!(async move { channel.send_session_reset().await });
        }

        // Note: Input is in seconds, a value of `0` means no timer.
        #[template_callback(function)]
        fn format_disappearing_messages_timer(time: u32) -> String {
            if time == 0 {
                return gettextrs::gettext("Never");
            }

            let weeks = if time >= 60 * 60 * 24 * 7 {
                Some(gettextrs::ngettext!(
                    "{} week",
                    "{} weeks",
                    time / (60 * 60 * 24 * 7),
                    time / (60 * 60 * 24 * 7)
                ))
            } else {
                None
            };
            let time = time % (60 * 60 * 24 * 7);

            let days = if time >= 60 * 60 * 24 {
                Some(gettextrs::ngettext!(
                    "{} day",
                    "{} days",
                    time / (60 * 60 * 24),
                    time / (60 * 60 * 24)
                ))
            } else {
                None
            };
            let time = time % (60 * 60 * 24);

            let hours = if time >= 60 * 60 {
                Some(gettextrs::ngettext!(
                    "{} hour",
                    "{} hours",
                    time / (60 * 60),
                    time / (60 * 60)
                ))
            } else {
                None
            };
            let time = time % (60 * 60);

            let minutes = if time >= 60 {
                Some(gettextrs::ngettext!(
                    "{} minute",
                    "{} minutes",
                    time / 60,
                    time / 60
                ))
            } else {
                None
            };
            let time = time % 60;

            let seconds = if time != 0 {
                Some(gettextrs::ngettext!("{} second", "{} seconds", time, time))
            } else {
                None
            };

            vec![weeks, hours, days, minutes, seconds]
                .into_iter()
                .flatten()
                // Temporarily collect the strings; `intersperse` would be nice.
                .collect::<Vec<_>>()
                .join(" ")
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChannelInfoDialog {
        const NAME: &'static str = "FlChannelInfoDialog";
        type Type = super::ChannelInfoDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
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

    impl AdwDialogImpl for ChannelInfoDialog {}
    impl WidgetImpl for ChannelInfoDialog {}
}
