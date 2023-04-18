use ashpd::{desktop::background::BackgroundRequest, WindowIdentifier};
use gdk::gdk_pixbuf::Pixbuf;
use gio::prelude::ApplicationExt;
use glib::{clone, Object};
use gtk::prelude::WidgetExt;
use gtk::{gdk, gio, glib};

use gettextrs::gettext;

use crate::gspawn;

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends libadwaita::PreferencesWindow, libadwaita::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PreferencesWindow {
    pub fn new() -> Self {
        Object::builder::<Self>().build()
    }

    async fn request_background(&self) -> ashpd::Result<()> {
        let identifier = WindowIdentifier::from_native(&self.native().unwrap()).await;
        let _ = BackgroundRequest::default()
            .reason(Some(
                gettext("Watch for new messages while closed").as_str(),
            ))
            .auto_start(true)
            .identifier(identifier)
            .command(&["flare", "--gapplication-service"])
            .dbus_activatable(false)
            .send()
            .await?;
        Ok(())
    }

    #[template_callback]
    fn on_background_switch_state_set(&self, state: bool) -> bool {
        let app = gio::Application::default().unwrap();
        if state {
            gspawn!(clone!(@weak self as this => async move {
                match this.request_background().await {
                    Ok(_) => {},
                    Err(err) => log::warn!("Failed to request background mode, {}", &err)
                }
            }));
        } else {
            let title = gettext("Background permission");
            let body = gettext("Use settings to remove permissions");
            let notification = gio::Notification::new(&title);
            notification.set_body(Some(body.as_str()));
            let icon =
                Pixbuf::from_resource("/icon.png").expect("Flare to have an application icon");
            notification.set_icon(&icon);
            app.send_notification(None, &notification);
        }
        false
    }
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}

pub mod imp {
    use gio::{Settings, SettingsBindFlags};
    use glib::subclass::InitializingObject;
    use gtk::{gio, glib};
    use gtk::{prelude::*, subclass::prelude::*, CompositeTemplate};
    use libadwaita::subclass::prelude::*;

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/preferences_window.ui")]
    pub struct PreferencesWindow {
        #[template_child]
        entry_device_name: TemplateChild<libadwaita::EntryRow>,

        #[template_child]
        switch_download_images: TemplateChild<gtk::Switch>,
        #[template_child]
        switch_download_videos: TemplateChild<gtk::Switch>,
        #[template_child]
        switch_download_files: TemplateChild<gtk::Switch>,

        #[template_child]
        switch_notifications: TemplateChild<gtk::Switch>,
        #[template_child]
        switch_background: TemplateChild<gtk::Switch>,

        #[template_child]
        switch_messages_selectable: TemplateChild<gtk::Switch>,

        settings: Settings,
    }

    impl PreferencesWindow {
        fn init_settings(&self) {
            self.settings
                .bind("link-device-name", &self.entry_device_name.get(), "text")
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-images",
                    &self.switch_download_images.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-videos",
                    &self.switch_download_videos.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-files",
                    &self.switch_download_files.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("notifications", &self.switch_notifications.get(), "active")
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("run-in-background", &self.switch_background.get(), "active")
                .flags(SettingsBindFlags::DEFAULT)
                .build();

            self.settings
                .bind(
                    "messages-selectable",
                    &self.switch_messages_selectable.get(),
                    "active",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "FlPreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = libadwaita::PreferencesWindow;

        fn new() -> Self {
            Self {
                settings: Settings::new(crate::config::APP_ID),
                entry_device_name: TemplateChild::default(),
                switch_download_images: TemplateChild::default(),
                switch_download_videos: TemplateChild::default(),
                switch_download_files: TemplateChild::default(),
                switch_notifications: TemplateChild::default(),
                switch_background: TemplateChild::default(),
                switch_messages_selectable: TemplateChild::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.bind_template_instance_callbacks();
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
    impl WindowImpl for PreferencesWindow {}
    impl PreferencesWindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {}
}
