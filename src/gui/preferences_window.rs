use crate::prelude::*;

use ashpd::{desktop::background::BackgroundRequest, WindowIdentifier};

use gettextrs::gettext;

glib::wrapper! {
    /// The preferences window of the application.
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PreferencesWindow {
    pub fn new() -> Self {
        Object::builder::<Self>().build()
    }

    async fn request_background(&self) -> ashpd::Result<()> {
        let identifier = WindowIdentifier::from_native(&self.native().unwrap()).await;
        tspawn!(async move {
            BackgroundRequest::default()
                .reason(Some(
                    gettext("Watch for new messages while closed").as_str(),
                ))
                .auto_start(true)
                .identifier(identifier)
                .command(&["flare", "--gapplication-service"])
                .dbus_activatable(false)
                .send()
                .await
                // Drop the result, otherwise it would be dropped in the glib thread. Due to dropping requiring tokio, this would panic.
                .map(|_| {})
        })
        .await
        .expect("Failed to join tokio")?;
        Ok(())
    }
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}

pub mod imp {
    use crate::prelude::*;

    use gdk_pixbuf::Pixbuf;
    use gettextrs::gettext;
    use gio::{Settings, SettingsBindFlags};
    use glib::subclass::InitializingObject;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/preferences_window.ui")]
    pub struct PreferencesWindow {
        #[template_child]
        row_download_images: TemplateChild<adw::SwitchRow>,
        #[template_child]
        row_download_videos: TemplateChild<adw::SwitchRow>,
        #[template_child]
        row_download_files: TemplateChild<adw::SwitchRow>,
        #[template_child]
        row_download_voice_messages: TemplateChild<adw::SwitchRow>,

        #[template_child]
        row_notifications: TemplateChild<adw::SwitchRow>,
        #[template_child]
        row_notifications_reactions: TemplateChild<adw::SwitchRow>,
        #[template_child]
        row_background: TemplateChild<adw::SwitchRow>,

        #[template_child]
        row_messages_selectable: TemplateChild<adw::SwitchRow>,
        #[template_child]
        row_send_on_enter: TemplateChild<adw::SwitchRow>,

        settings: Settings,
    }

    #[gtk::template_callbacks]
    impl PreferencesWindow {
        #[template_callback]
        fn on_background_switch_state_set(&self, _: glib::ParamSpec, r: adw::SwitchRow) {
            let obj = self.obj();
            let state = r.is_active();
            let app = gio::Application::default().unwrap();
            if state {
                gspawn!(clone!(
                    #[weak]
                    obj,
                    async move {
                        match obj.request_background().await {
                            Ok(_) => {}
                            Err(err) => log::warn!("Failed to request background mode, {}", &err),
                        }
                    }
                ));
            } else {
                let title = gettext("Background permission");
                let body = gettext("Use settings to remove permissions");
                let notification = gio::Notification::new(&title);
                notification.set_body(Some(body.as_str()));
                let icon =
                    Pixbuf::from_resource("/icon.svg").expect("Flare to have an application icon");
                notification.set_icon(&icon);
                app.send_notification(None, &notification);
            }
        }

        fn init_settings(&self) {
            self.settings
                .bind(
                    "autodownload-images",
                    &self.row_download_images.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-videos",
                    &self.row_download_videos.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-files",
                    &self.row_download_files.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-voice-messages",
                    &self.row_download_voice_messages.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("notifications", &self.row_notifications.get(), "active")
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "notify-reactions",
                    &self.row_notifications_reactions.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("run-in-background", &self.row_background.get(), "active")
                .flags(SettingsBindFlags::DEFAULT)
                .build();

            self.settings
                .bind(
                    "messages-selectable",
                    &self.row_messages_selectable.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("send-on-enter", &self.row_send_on_enter.get(), "active")
                .flags(SettingsBindFlags::DEFAULT)
                .build();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "FlPreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesDialog;

        fn new() -> Self {
            Self {
                settings: Settings::new(crate::config::BASE_ID),
                row_download_images: TemplateChild::default(),
                row_download_videos: TemplateChild::default(),
                row_download_voice_messages: TemplateChild::default(),
                row_download_files: TemplateChild::default(),
                row_notifications: TemplateChild::default(),
                row_notifications_reactions: TemplateChild::default(),
                row_background: TemplateChild::default(),
                row_messages_selectable: TemplateChild::default(),
                row_send_on_enter: TemplateChild::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.init_settings();
        }
    }
    impl WidgetImpl for PreferencesWindow {}
    impl PreferencesDialogImpl for PreferencesWindow {}
    impl AdwDialogImpl for PreferencesWindow {}
}
