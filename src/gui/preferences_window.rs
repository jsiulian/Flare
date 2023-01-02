use gdk::glib::Object;
use glib::clone;
use gtk::prelude::WidgetExt;
use gdk_pixbuf::Pixbuf;
use gio::prelude::ApplicationExt;
use ashpd::desktop::background;

use gettextrs::gettext;

use crate::gspawn;

gtk::glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends libadwaita::PreferencesWindow, libadwaita::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PreferencesWindow {
    pub fn new() -> Self {
        Object::new(&[]).expect("Failed to create PreferencesWindow")
    }

    async fn request_background(&self) -> ashpd::Result<()> {
        let identifier = ashpd::WindowIdentifier::from_native(
            &self.native().unwrap()
        ).await;
        let _ = background::request(
            &identifier,
            &gettext("Watch for new messages while closed"),
            true,
            Some(&["flare", "--gapplication-service"]),
            false,
        )
        .await?;

        Ok(())
    }

    #[template_callback]
    fn on_background_switch_state_set(&self, state: bool) -> bool {
        let app =  gio::Application::default().unwrap();
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
            let icon = Pixbuf::from_resource("/icon.png").expect("Flare to have an application icon");
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
        spin_initial_message_loading: TemplateChild<gtk::SpinButton>,
        #[template_child]
        spin_request_message_loading: TemplateChild<gtk::SpinButton>,

        #[template_child]
        switch_notifications: TemplateChild<gtk::Switch>,
        #[template_child]
        switch_background: TemplateChild<gtk::Switch>,

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
                    "state",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-videos",
                    &self.switch_download_videos.get(),
                    "state",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "autodownload-files",
                    &self.switch_download_files.get(),
                    "state",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "messages-initial-load",
                    &self.spin_initial_message_loading.get(),
                    "value",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind(
                    "messages-request-load",
                    &self.spin_request_message_loading.get(),
                    "value",
                )
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("notifications", &self.switch_notifications.get(), "state")
                .flags(SettingsBindFlags::DEFAULT)
                .build();
            self.settings
                .bind("run-in-background", &self.switch_background.get(), "state")
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
                spin_initial_message_loading: TemplateChild::default(),
                spin_request_message_loading: TemplateChild::default(),
                switch_notifications: TemplateChild::default(),
                switch_background: TemplateChild::default(),
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
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.init_settings();
        }
    }
    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl PreferencesWindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {}
}
