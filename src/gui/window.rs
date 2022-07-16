use gtk::glib::Object;

gtk::glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends libadwaita::ApplicationWindow, gtk::ApplicationWindow, libadwaita::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &gtk::Application) -> Self {
        log::trace!("Initializing window");
        Object::new(&[("application", app)]).expect("Failed to create Window")
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
    use glib::subclass::InitializingObject;
    use gtk::builders::AboutDialogBuilder;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;
    use libadwaita::subclass::prelude::AdwApplicationWindowImpl;
    use libadwaita::subclass::prelude::AdwWindowImpl;

    use crate::backend::Manager;
    use crate::gui::error_dialog::ErrorDialog;
    use crate::gui::link_window::LinkWindow;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/window.ui")]
    pub struct Window {
        #[template_child]
        leaflet: TemplateChild<libadwaita::Leaflet>,

        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl Window {
        fn setup_actions(&self, obj: &super::Window) {
            log::trace!("Setting up window actions");
            log::trace!("Setting up preferences-window action");
            let action_settings = SimpleAction::new("settings", None);
            // TODO
            // action_settings.connect_activate(|_, _| {
            //     let settings = PreferencesWindow::new();
            //     settings.show();
            // });
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
            actions.add_action(&action_about);
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
                    return;
                }
                // TODO: Move init to after message receive
                manager.init_channels().await;
                if let Err(e) = manager.setup_receive_message_loop().await {
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
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}
