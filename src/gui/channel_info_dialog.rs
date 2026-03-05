use crate::prelude::*;

use crate::backend::{Channel, Manager};

glib::wrapper! {
    /// Dialog showing more information about the channel.
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
}

pub mod imp {
    use crate::prelude::*;
    #[cfg(target_os = "linux")]
    use ashpd::WindowIdentifier;
    #[cfg(target_os = "linux")]
    use ashpd::desktop::open_uri::OpenFileRequest;
    use gtk::Align;

    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;

    use crate::backend::{Channel, Manager};

    #[derive(CompositeTemplate, Default, glib::Properties)]
    #[properties(wrapper_type = super::ChannelInfoDialog)]
    #[template(resource = "/ui/channel_info_dialog.ui")]
    pub struct ChannelInfoDialog {
        #[template_child]
        avatar: TemplateChild<adw::Avatar>,
        #[template_child]
        row_phone: TemplateChild<adw::ActionRow>,
        #[template_child]
        row_disappearing: TemplateChild<adw::ActionRow>,
        #[template_child]
        row_description: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        group_phone_description: TemplateChild<adw::PreferencesGroup>,

        #[property(get, set, construct_only, type = Channel)]
        channel: RefCell<Option<Channel>>,
        #[property(get, set, construct_only, type = Manager)]
        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl ChannelInfoDialog {
        // By default, the icon is centered. This looks weird with a long description.
        fn fixup_description_expander_row_icon(&self) {
            let icon = self
                .row_description
                .first_child() // box
                .and_then(|w| w.first_child()) // ListBox
                .and_then(|w| w.first_child()) // action_row
                .and_then(|w| w.first_child()) // header
                .and_then(|w| w.last_child()) // suffixes
                .and_then(|w| w.last_child()); // image

            if let Some(icon) = icon {
                icon.set_margin_top(18);
                icon.set_valign(Align::Start);
            } else {
                log::warn!("Cannot fix up description expander row of channel info dialog");
            }
        }
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
            let single_line_description = channel.single_line_description();
            self.row_description.set_visible(description.is_some());
            self.row_description.set_subtitle(
                single_line_description
                    .as_ref()
                    .unwrap_or(&String::default()),
            );
            self.row_description.set_subtitle_lines(1);

            self.row_description.connect_expanded_notify(move |row| {
                if row.is_expanded() {
                    row.set_subtitle(description.as_ref().unwrap_or(&String::default()));
                    row.set_subtitle_lines(0);
                } else {
                    row.set_subtitle(
                        single_line_description
                            .as_ref()
                            .unwrap_or(&String::default()),
                    );
                    row.set_subtitle_lines(1);
                }
            });

            self.group_phone_description
                .set_visible(self.row_phone.is_visible() || self.row_description.is_visible());
        }

        #[template_callback]
        fn reset_session(&self) {
            let obj = self.obj();
            let channel = obj.channel();
            crate::trace!("Resetting session of channel {}", channel.title());
            gspawn!(async move { channel.send_session_reset().await });
        }

        #[template_callback]
        fn open_phone_number(&self) {
            #[cfg(target_os = "linux")]
            let obj = self.obj();
            let channel = self.channel.borrow();
            let phone_number = channel.as_ref().and_then(|c| c.phone_number());

            if let Some(url) = phone_number.and_then(|p| url::Url::parse(&format!("tel:{p}")).ok())
            {
                #[cfg(target_os = "linux")]
                gspawn!(clone!(
                    #[weak]
                    obj,
                    async move {
                        let identifier =
                            WindowIdentifier::from_native(&obj.native().unwrap()).await;
                        tspawn!(async move {
                            if let Err(e) = OpenFileRequest::default()
                                .identifier(identifier)
                                .send_uri(&url)
                                .await
                            {
                                log::error!("Failed to open phone number: {}", e);
                            }
                        })
                        .await
                        .expect("Failed to join tokio")
                    }
                ));

                #[cfg(not(target_os = "linux"))]
                if let Err(e) =
                    gio::AppInfo::launch_default_for_uri(url.as_str(), gio::AppLaunchContext::NONE)
                {
                    log::error!("Failed to open phone number: {}", e);
                }
            } else {
                log::warn!("Trying to open phone number even if it does not exist");
            }
        }

        // Note: Input is in seconds, a value of `0` means no timer.
        #[template_callback(function)]
        fn format_disappearing_messages_timer(time: u32) -> String {
            if time == 0 {
                return gettextrs::gettext("Never");
            }

            let weeks = if time >= 60 * 60 * 24 * 7 {
                Some(
                    gettextrs::ngettext("{} week", "{} weeks", time / (60 * 60 * 24 * 7))
                        .replace("{}", &(time / (60 * 60 * 24 * 7)).to_string()),
                )
            } else {
                None
            };
            let time = time % (60 * 60 * 24 * 7);

            let days = if time >= 60 * 60 * 24 {
                Some(
                    gettextrs::ngettext("{} day", "{} days", time / (60 * 60 * 24))
                        .replace("{}", &(time / (60 * 60 * 24)).to_string()),
                )
            } else {
                None
            };
            let time = time % (60 * 60 * 24);

            let hours = if time >= 60 * 60 {
                Some(
                    gettextrs::ngettext("{} hour", "{} hours", time / (60 * 60))
                        .replace("{}", &(time / (60 * 60)).to_string()),
                )
            } else {
                None
            };
            let time = time % (60 * 60);

            let minutes = if time >= 60 {
                Some(
                    gettextrs::ngettext("{} minute", "{} minutes", time / 60)
                        .replace("{}", &(time / 60).to_string()),
                )
            } else {
                None
            };
            let time = time % 60;

            let seconds = if time != 0 {
                Some(
                    gettextrs::ngettext("{} second", "{} seconds", time)
                        .replace("{}", &time.to_string()),
                )
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

    #[glib::derived_properties]
    impl ObjectImpl for ChannelInfoDialog {
        fn constructed(&self) {
            self.parent_constructed();
            self.fixup_description_expander_row_icon();
        }
    }

    impl AdwDialogImpl for ChannelInfoDialog {}
    impl WidgetImpl for ChannelInfoDialog {}
}
