use gdk::glib::Object;

gtk::glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends libadwaita::PreferencesWindow, libadwaita::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PreferencesWindow {
    pub fn new() -> Self {
        Object::new(&[]).expect("Failed to create PreferencesWindow")
    }
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}

pub mod imp {
    use gdk::gio::Settings;
    use gdk::gio::SettingsBindFlags;
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;
    use libadwaita::subclass::prelude::AdwWindowImpl;
    use libadwaita::subclass::prelude::PreferencesWindowImpl;

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
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
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
