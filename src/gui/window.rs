use gdk::subclass::prelude::ObjectSubclassIsExt;
use gdk_pixbuf::{glib, prelude::SettingsExt};
use gtk::prelude::GtkApplicationExt;
use gtk::{glib::Object, traits::GtkWindowExt};

gtk::glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends libadwaita::ApplicationWindow, gtk::ApplicationWindow, libadwaita::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &libadwaita::Application) -> Self {
        log::trace!("Initializing window");
        app.set_accels_for_action("win.settings", &["<Control>comma"]);
        app.set_accels_for_action("win.show-help-overlay", &["<Control>question"]);
        app.set_accels_for_action("win.about", &["F1"]);
        app.set_accels_for_action("channel-messages.activate-input", &["<Control>i"]);
        app.set_accels_for_action("channel-messages.load-more", &["<Control>l"]);
        for i in 1..=9 {
            app.set_accels_for_action(
                &format!("channel-list.activate-channel({})", i),
                &[&format!("<Control>{}", i)],
            );
        }
        app.set_accels_for_action(&"channel-list.toggle-search", &[&"<Control>f"]);
        Object::new(&[("application", app)]).expect("Failed to create Window")
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        let (width, height) = self.default_size();

        imp.settings.set_int("window-width", width)?;
        imp.settings.set_int("window-height", height)?;

        imp.settings
            .set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let imp = self.imp();

        let width = imp.settings.int("window-width");
        let height = imp.settings.int("window-height");
        let is_maximized = imp.settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }
}

pub mod imp {
    use std::cell::RefCell;
    use std::env;
    use std::path::PathBuf;

    use gdk::gio::SimpleAction;
    use gdk::gio::SimpleActionGroup;
    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::MainContext;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::Value;
    use gio::Settings;
    use glib::subclass::InitializingObject;
    use gtk::builders::AboutDialogBuilder;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::Builder;
    use gtk::CompositeTemplate;
    use gtk::ShortcutsWindow;
    use libadwaita::subclass::prelude::AdwApplicationWindowImpl;
    use libadwaita::subclass::prelude::AdwWindowImpl;

    use crate::backend::Manager;
    use crate::config::APP_ID;
    use crate::gui::channel_list::ChannelList;
    use crate::gui::channel_messages::ChannelMessages;
    use crate::gui::error_dialog::ErrorDialog;
    use crate::gui::link_window::LinkWindow;
    use crate::gui::preferences_window::PreferencesWindow;

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/window.ui")]
    pub struct Window {
        #[template_child]
        leaflet: TemplateChild<libadwaita::Leaflet>,

        #[template_child]
        channel_list: TemplateChild<ChannelList>,
        #[template_child]
        channel_messages: TemplateChild<ChannelMessages>,

        manager: RefCell<Option<Manager>>,

        pub(super) settings: gio::Settings,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                leaflet: Default::default(),
                channel_list: Default::default(),
                channel_messages: Default::default(),
                manager: Default::default(),
                settings: Settings::new(APP_ID),
            }
        }
    }

    #[gtk::template_callbacks]
    impl Window {
        fn setup_actions(&self, obj: &super::Window) {
            log::trace!("Setting up window actions");
            log::trace!("Setting up preferences-window action");
            let action_settings = SimpleAction::new("settings", None);
            action_settings.connect_activate(|_, _| {
                let settings = PreferencesWindow::new();
                settings.show();
            });
            log::trace!("Setting up unlink action");
            let action_unlink = SimpleAction::new("unlink", None);
            action_unlink.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("User requested to unlink the device");
                // TODO: AdwMessageDialog
                let dialog = gtk::MessageDialog::builder()
                    .buttons(gtk::ButtonsType::OkCancel)
                    .message_type(gtk::MessageType::Warning)
                    .secondary_text(&gettextrs::gettext(
                        "This will unlink this device and remove all locally saved data. This will also close the application such that it can be relinked when opening it again.",
                    ))
                    .text(&gettextrs::gettext("Are you sure?"))
                    .transient_for(&obj)
                    .build();
                dialog.connect_response(clone!(@weak obj => move |dialog, response| {
                    dialog.close();
                    if response == gtk::ResponseType::Ok {
                        log::info!("Unlinking device");
                        if let Some(man) = obj.imp().manager.borrow().as_ref() {
                            if let Err(e)= man.clear() {
                                log::error!("Failed to clear db: {}", e);
                            }
                        }
                        obj.close();
                    }
                }));
                dialog.show();
            }));

            let action_show_help_overlay = SimpleAction::new("show-help-overlay", None);
            action_show_help_overlay.connect_activate(|_, _| {
                let builder = Builder::from_resource("/ui/shortcuts.ui");
                let shortcuts_window: ShortcutsWindow = builder
                    .object("help_overlay")
                    .expect("shortcuts.ui to have at least one object help_overlay");
                shortcuts_window.show();
            });

            log::trace!("Setting up about-page action");
            let action_about = SimpleAction::new("about", None);
            action_about.connect_activate(|_, _| {
                let about_dialog = AboutDialogBuilder::new()
                    .authors(
                        env!("CARGO_PKG_AUTHORS")
                            .split(';')
                            .map(|s| s.to_string())
                            .collect(),
                    )
                    .comments(env!("CARGO_PKG_DESCRIPTION"))
                    .copyright(
                        include_str!("../../NOTICE")
                            .to_string()
                            .lines()
                            .next()
                            .unwrap_or_default(),
                    )
                    .license_type(gtk::License::Gpl30)
                    .logo_icon_name("icon")
                    .program_name("Flare")
                    // Translators: Put your name contact info here if you want to be credited in
                    // the about page.
                    .translator_credits(&gettextrs::gettext("translators"))
                    .version(crate::config::VERSION)
                    .website(env!("CARGO_PKG_HOMEPAGE"))
                    .build();
                about_dialog.show();
            });

            log::trace!("Adding a action to the group");
            let actions = SimpleActionGroup::new();
            obj.insert_action_group("win", Some(&actions));
            actions.add_action(&action_settings);
            actions.add_action(&action_unlink);
            actions.add_action(&action_show_help_overlay);
            actions.add_action(&action_about);

            let action_activate_input = SimpleAction::new("activate-input", None);
            action_activate_input.connect_activate(
                clone!(@strong self.channel_messages as channel_messages => move |_, _| {
                    channel_messages.focus_input();
                }),
            );
            let action_load_more = SimpleAction::new("load-more", None);
            action_load_more.connect_activate(
                clone!(@strong self.channel_messages as channel_messages => move |_, _| {
                    channel_messages.load_more();
                }),
            );
            let actions = SimpleActionGroup::new();
            obj.insert_action_group("channel-messages", Some(&actions));
            actions.add_action(&action_activate_input);
            actions.add_action(&action_load_more);

            let action_activate_channel =
                SimpleAction::new("activate-channel", Some(&i32::static_variant_type()));
            action_activate_channel.connect_activate(
                clone!(@strong self.channel_list as channel_list => move |_, parameter| {
                    let parameter = parameter
                        .expect("Could not get parameter.")
                        .get::<i32>()
                        .expect("The variant needs to be of type `i32`.");
                    channel_list.activate_row((parameter - 1).try_into().unwrap_or_default());
                }),
            );
            let action_toggle_search = SimpleAction::new("toggle-search", None);
            action_toggle_search.connect_activate(
                clone!(@strong self.channel_list as channel_list => move |_, _| {
                    channel_list.toggle_search();
                }),
            );
            let actions = SimpleActionGroup::new();
            obj.insert_action_group("channel-list", Some(&actions));
            actions.add_action(&action_activate_channel);
            actions.add_action(&action_toggle_search);
        }

        #[template_callback]
        fn handle_search_clicked(&self) {
            self.channel_list.toggle_search();
        }

        #[template_callback]
        fn handle_go_back(&self) {
            log::trace!("Go backward in the leaflet");
            self.leaflet.navigate(libadwaita::NavigationDirection::Back);
        }

        #[template_callback]
        fn handle_go_forward(&self) {
            log::trace!("Go forward in the leaflet");
            self.leaflet
                .navigate(libadwaita::NavigationDirection::Forward);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "FlWindow";
        type Type = super::Window;
        type ParentType = libadwaita::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            crate::gui::channel_list::ChannelList::ensure_type();
            crate::gui::channel_messages::ChannelMessages::ensure_type();
            crate::gui::link_window::LinkWindow::ensure_type();
            crate::gui::error_dialog::ErrorDialog::ensure_type();
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self, obj: &Self::Type) {
            log::trace!("Constructed window");
            self.parent_constructed(obj);
            self.setup_actions(obj);

            // Devel Profile
            if crate::config::PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            obj.load_window_size();

            let main_context = MainContext::default();
            main_context.spawn_local(clone!(@strong obj => async move {
                log::trace!("Constructing path for configuration");
                let path = PathBuf::from(
                    env::var("FLARE_DATA_PATH").unwrap_or_else(|_| 
                        env::var("XDG_DATA_HOME")
                            .map(|s| s + "/flare/")
                            .unwrap_or_else(|_| env::var("HOME").map(|s| s + "/.local/share/flare/").expect("Could not find $HOME")),
                    ),
                );
                log::trace!("Setup manager for Window");
                let manager = Manager::new();
                obj.set_property("manager", Some(&manager));
                manager.connect_local("link-qr-code", false, clone!(@weak obj => @default-return None, move |args| {
                    let man = args[0]
                        .get::<Manager>()
                        .expect("First argument of signal `link-qr-code` of `Manager` to be `Manager`");
                    let url = args[1]
                        .get::<String>()
                        .expect("Second argument of signal `link-qr-code` of `Manager` to be `String`");
                    crate::trace!("Opening link window for url {}", url);
                    let window = LinkWindow::new(url, man, &obj);
                    window.show();
                    None
                }));

                if let Err(e) = manager.init(&path).await {
                    let dialog = ErrorDialog::new(e, &obj);
                    dialog.show();
                }
            }));
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecObject::new(
                    "manager",
                    "manager",
                    "manager",
                    Manager::static_type(),
                    ParamFlags::READWRITE,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Window` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self, window: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = window.save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            self.parent_close_request(window)
        }
    }
    impl ApplicationWindowImpl for Window {}
    impl AdwWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}
