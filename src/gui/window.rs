use gdk::{gio, glib, prelude::SettingsExt, subclass::prelude::*};
use glib::Object;
use gtk::prelude::*;

use crate::backend::Manager;

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        log::trace!("Initializing window");
        app.set_accels_for_action("win.settings", &["<Control>comma"]);
        app.set_accels_for_action("win.show-help-overlay", &["<Control>question"]);
        app.set_accels_for_action("win.about", &["F1"]);
        app.set_accels_for_action("window.close", &["<Control>q"]);
        app.set_accels_for_action("channel-messages.activate-input", &["<Control>i"]);
        app.set_accels_for_action("channel-messages.load-more", &["<Control>l"]);
        for i in 1..=9 {
            app.set_accels_for_action(
                &format!("channel-list.activate-channel({})", i),
                &[&format!("<Control>{}", i)],
            );
        }
        app.set_accels_for_action("channel-list.toggle-search", &["<Control>f"]);
        Object::builder::<Self>()
            .property("application", app)
            .build()
    }

    // Closes the window, even if hide-on-close is set.
    pub fn kill(&self) {
        self.set_hide_on_close(false);
        self.close();
    }

    pub fn destroy_if_invisible(&self) {
        if !self.get_visible() {
            self.destroy();
        }
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

    fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub(crate) fn settings(&self) -> gio::Settings {
        self.imp().settings.clone()
    }
}

pub mod imp {
    use std::{cell::RefCell, env, path::PathBuf};

    use adw::{prelude::*, subclass::prelude::*, AboutDialog};
    use adw::{AlertDialog, EntryRow, ResponseAppearance};
    use gdk::gio::Cancellable;
    use gdk::glib::{BindingFlags, BoxedAnyObject, Propagation};
    use gio::{Settings, SimpleAction, SimpleActionGroup};
    use glib::{clone, subclass::InitializingObject, ParamSpec, ParamSpecObject, Value};
    use gtk::{gio, glib};
    use gtk::{Builder, CompositeTemplate, ShortcutsWindow};
    use once_cell::sync::Lazy;

    use crate::backend::{Channel, SetupResult};
    use crate::gui::channel_info_dialog::ChannelInfoDialog;
    use crate::gui::linked_devices_window::LinkedDevicesWindow;
    use crate::gui::new_channel_dialog::NewChannelDialog;
    use crate::{
        backend::Manager,
        config::APP_ID,
        gspawn,
        gui::{
            channel_list::ChannelList, channel_messages::ChannelMessages,
            error_dialog::ErrorDialog, preferences_window::PreferencesWindow,
            setup_window::SetupWindow,
        },
    };

    #[derive(CompositeTemplate)]
    #[template(resource = "/ui/window.ui")]
    pub struct Window {
        #[template_child]
        split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub(super) channel_list: TemplateChild<ChannelList>,
        #[template_child]
        subtitle_label: TemplateChild<gtk::Label>,
        #[template_child]
        channel_messages: TemplateChild<ChannelMessages>,
        #[template_child]
        new_channel_dialog: TemplateChild<NewChannelDialog>,

        manager: RefCell<Option<Manager>>,

        pub(super) settings: gio::Settings,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                split_view: Default::default(),
                channel_list: Default::default(),
                subtitle_label: Default::default(),
                channel_messages: Default::default(),
                new_channel_dialog: Default::default(),
                manager: Default::default(),
                settings: Settings::new(APP_ID),
            }
        }
    }

    #[gtk::template_callbacks]
    impl Window {
        fn setup_actions(&self) {
            log::trace!("Setting up window actions");
            log::trace!("Setting up preferences-window action");
            let obj = self.obj();
            let action_settings = SimpleAction::new("settings", None);
            action_settings.connect_activate(clone!(@weak obj => move |_, _| {
                let settings = PreferencesWindow::new();
                settings.present(&obj);
            }));

            let action_clear_messages = SimpleAction::new("clear-messages", None);
            action_clear_messages.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("User requested to clear messages");
                let builder = Builder::from_resource("/ui/dialog_clear_messages.ui");
                let dialog: AlertDialog = builder
                    .object("dialog")
                    .expect("dialog_clear_messages.ui to have at least one object dialog");
                dialog.connect_response(None, clone!(@weak obj => move |_dialog, response| {
                    if response == "clear" {
                        log::info!("Clear messages device");
                        if let Some(man) = obj.imp().manager.borrow().as_ref() {
                            if let Err(e) = man.clear_messages() {
                                log::error!("Failed to clear db: {}", e);
                            }
                        }
                        log::trace!("Closing the window after clear");
                        obj.kill();
                    }
                }));
                dialog.present(&obj);
            }));
            log::trace!("Setting up unlink action");
            let action_unlink = SimpleAction::new("unlink", None);
            action_unlink.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("User requested to unlink the device");
                let builder = Builder::from_resource("/ui/dialog_unlink.ui");
                let dialog: AlertDialog = builder
                    .object("dialog")
                    .expect("dialog_unlink.ui to have at least one object dialog");
                dialog.connect_response(None, clone!(@weak obj => move |_dialog, response| {
                    if response == "unlink-keep" {
                        log::info!("Unlinking device");
                        if let Some(man) = obj.imp().manager.borrow().as_ref() {
                            if let Err(e) = man.clear_registration() {
                                log::error!("Failed to clear db: {}", e);
                            }
                        }
                        log::trace!("Closing the window after unlink");
                        obj.kill();
                    } else if response == "unlink-delete" {
                        log::info!("Unlinking device");
                        if let Some(man) = obj.imp().manager.borrow().as_ref() {
                            if let Err(e) = man.clear_registration() {
                                log::error!("Failed to clear db: {}", e);
                            }
                            if let Err(e) = man.clear_contacts() {
                                log::error!("Failed to clear db: {}", e);
                            }
                            if let Err(e) = man.clear_groups() {
                                log::error!("Failed to clear db: {}", e);
                            }
                            if let Err(e) = man.clear_messages() {
                                log::error!("Failed to clear db: {}", e);
                            }
                        }
                        log::trace!("Closing the window after unlink");
                        obj.kill();
                    }
                }));
                dialog.present(&obj);
            }));

            log::trace!("Setting up submit-captcha action");
            let action_submit_captcha = SimpleAction::new("submit-captcha", None);
            action_submit_captcha.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("User requested to submit a captcha the device");
                let builder = Builder::from_resource("/ui/submit_captcha_dialog.ui");
                let dialog: AlertDialog = builder
                    .object("dialog")
                    .expect("submit_captcha_dialog.ui to have at least one object dialog");
                let entry_token: EntryRow = builder
                    .object("entry_token")
                    .expect("submit_captcha_dialog.ui to have at least one object entry_token");
                let entry_captcha: EntryRow = builder
                    .object("entry_captcha")
                    .expect("submit_captcha_dialog.ui to have at least one object entry_captcha");
                dialog.connect_response(None, clone!(@weak obj, @weak entry_token, @weak entry_captcha => move |_dialog, response| {
                    if response == "submit" {
                        log::info!("Unlinking device");
                        if let Some(man) = obj.imp().manager.borrow().as_ref() {
                            let token = entry_token.text();
                            let captcha = entry_captcha.text();
                            gspawn!(clone!(@weak man => async move {
                                if let Err(e) = man.submit_recaptcha_challenge(&token, &captcha).await {
                                    // TODO: Show error dialog?
                                    log::error!("Failed to submit recaptcha: {}", e);
                                }
                            }));
                        }
                    }
                }));
                dialog.present(&obj);
            }));
            log::trace!("Setting up sync-contacts action");
            let action_sync_contacts = SimpleAction::new("sync-contacts", None);
            action_sync_contacts.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("User requested to synchronize contacts");
                if let Some(man) = obj.imp().manager.borrow().as_ref() {
                    gspawn!(clone!(@weak man => async move {
                        if let Err(e) = man.request_contacts_sync().await {
                            // TODO: Show error dialog?
                            log::error!("Failed to synchronize contacts: {}", e);
                        }
                    }));
                };
            }));

            let action_show_help_overlay = SimpleAction::new("show-help-overlay", None);
            action_show_help_overlay.connect_activate(|_, _| {
                let builder = Builder::from_resource("/ui/shortcuts.ui");
                let shortcuts_window: ShortcutsWindow = builder
                    .object("help_overlay")
                    .expect("shortcuts.ui to have at least one object help_overlay");
                shortcuts_window.present();
            });

            log::trace!("Setting up about-page action");
            let action_about = SimpleAction::new("about", None);
            action_about.connect_activate(clone!(@weak obj => move |_, _| {
                let builder = Builder::from_resource("/ui/about.ui");
                let about: AboutDialog = builder
                    .object("about")
                    .expect("about.ui to have at least one object about");
                // TODO: Replace this in Blueprint when string[] is supported
                about.set_artists(&["David Lapshin <ddaudix@gmail.com>"]);
                about.add_link("GitLab", "https://gitlab.com/schmiddi-on-mobile/flare");
                about.present(&obj);
            }));

            log::trace!("Setting up kill action");
            let action_kill = SimpleAction::new("kill", None);
            action_kill.connect_activate(clone!(@weak obj => move |_, _| {
                obj.kill();
            }));

            log::trace!("Setting up channel information action");
            let action_channel_information = SimpleAction::new("channel-information", None);
            action_channel_information.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("Requested channel info");
                let Some(channel) = obj.imp().channel_messages.active_channel() else {return};
                let channel_info = ChannelInfoDialog::new(&channel, &obj.manager());
                channel_info.present(&obj);
            }));

            log::trace!("Setting up channel clear messages action");
            let action_channel_clear_messages = SimpleAction::new("channel-clear-messages", None);
            action_channel_clear_messages.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("Requested clearing messages of channels");
                let confirmation_dialog = AlertDialog::builder()
                    .heading(gettextrs::gettext("Remove Messages"))
                    .body(gettextrs::gettext("This will remove all locally stored messages from this channel"))
                    .close_response("cancel")
                    .default_response("cancel")
                    .build();
                confirmation_dialog.add_response("cancel",  &gettextrs::gettext("Cancel"));
                confirmation_dialog.add_response("remove",  &gettextrs::gettext("Remove Messages"));
                confirmation_dialog.set_response_appearance("remove", ResponseAppearance::Destructive);
                confirmation_dialog.choose(&obj, None::<&Cancellable>, clone!(@weak obj => move |response| {
                    if response == "remove" {
                        if let Err(e) = obj.imp().channel_messages.clear_messages() {
                            let dialog = ErrorDialog::new(e, &obj);
                            dialog.present(&obj);
                        }
                    }
                }));
            }));

            self.channel_messages
                .bind_property("active-channel", &action_channel_information, "enabled")
                .transform_to(|_, c: Option<Channel>| Some(c.is_some()))
                .flags(BindingFlags::SYNC_CREATE)
                .build();

            self.channel_messages
                .bind_property("active-channel", &action_channel_clear_messages, "enabled")
                .transform_to(|_, c: Option<Channel>| Some(c.is_some()))
                .flags(BindingFlags::SYNC_CREATE)
                .build();

            log::trace!("Adding a action to the group");
            let actions = SimpleActionGroup::new();
            obj.insert_action_group("win", Some(&actions));
            actions.add_action(&action_settings);
            actions.add_action(&action_clear_messages);
            actions.add_action(&action_unlink);
            actions.add_action(&action_submit_captcha);
            actions.add_action(&action_sync_contacts);
            actions.add_action(&action_show_help_overlay);
            actions.add_action(&action_about);
            actions.add_action(&action_kill);
            actions.add_action(&action_channel_information);
            actions.add_action(&action_channel_clear_messages);

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

        // Requires the manager to be set up. Therefore, postponed.
        fn setup_linked_devices_action(&self) {
            let obj = self.obj();

            log::trace!("Setting up linked-devices action");
            let action_linked_devices = SimpleAction::new("linked-devices", None);
            action_linked_devices.connect_activate(clone!(@weak obj => move |_, _| {
                log::trace!("User requested to view linked devices");
                let win = LinkedDevicesWindow::new(obj.manager(), &obj);
                win.present(&obj);
            }));

            self.obj()
                .manager()
                .bind_property("is-primary", &action_linked_devices, "enabled")
                .flags(BindingFlags::SYNC_CREATE)
                .build();

            let actions = SimpleActionGroup::new();
            obj.insert_action_group("win-managed", Some(&actions));
            actions.add_action(&action_linked_devices);
        }

        #[template_callback]
        fn handle_search_clicked(&self) {
            self.channel_list.toggle_search();
        }

        #[template_callback]
        fn handle_go_back(&self) {
            log::trace!("Go backward in the SplitView");
            self.split_view.set_show_content(false);
        }

        #[template_callback]
        fn handle_go_forward(&self) {
            log::trace!("Go forward in the SplitView");
            self.split_view.set_show_content(true);
        }

        #[template_callback]
        fn handle_typing(&self, is_typing: bool, description: Option<String>) -> Option<String> {
            if is_typing {
                self.subtitle_label.add_css_class("accent");
                self.subtitle_label.remove_css_class("dim-label");
                Some(gettextrs::gettext("is typing"))
            } else {
                self.subtitle_label.remove_css_class("accent");
                self.subtitle_label.add_css_class("dim-label");
                description
            }
        }

        #[template_callback]
        fn handle_add_conversation_clicked(&self) {
            let dialog = &self.new_channel_dialog;
            dialog.present_for_selection(&self.obj().manager().available_channels());
        }

        #[template_callback]
        fn handle_new_channel(&self, channel: Channel) {
            self.channel_list.add_channel(channel);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "FlWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            crate::gui::channel_list::ChannelList::ensure_type();
            crate::gui::channel_messages::ChannelMessages::ensure_type();
            crate::gui::setup_window::SetupWindow::ensure_type();
            crate::gui::error_dialog::ErrorDialog::ensure_type();
            crate::backend::timeline::TimelineItem::ensure_type();
            crate::gui::new_channel_dialog::NewChannelDialog::ensure_type();
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            crate::gui::utility::Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            log::trace!("Constructed window");
            let obj = self.obj();
            self.parent_constructed();
            obj.imp().setup_actions();

            // Devel Profile
            if crate::config::PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            obj.load_window_size();

            gspawn!(clone!(@strong obj => async move {
                log::trace!("Constructing path for configuration");
                let path = PathBuf::from(
                    env::var("FLARE_DATA_PATH").unwrap_or_else(|_|
                        env::var("XDG_DATA_HOME")
                            .map(|s| s + "/flare/")
                            .unwrap_or_else(|_| env::var("HOME").map(|s| s + "/.local/share/flare/").expect("Could not find $HOME")),
                    ),
                );
                log::trace!("Setup manager for Window");
                let manager = Manager::new(obj.property::<gio::Application>("application"));
                obj.set_property("manager", Some(&manager));
                obj.imp().setup_linked_devices_action();

                let setup_window: RefCell<Option<SetupWindow>> = RefCell::default();

                manager.connect_local(
                    "setup-result",
                    false,
                    clone!(@weak obj, @weak manager, @strong setup_window => @default-return None, move |r| {
                        // r[0] is the manager
                        let result = r[1].get::<BoxedAnyObject>().expect("Setup-Result to be BoxedAnyObject");
                        let result: &mut SetupResult = &mut result.borrow_mut();
                        let mut setup_window = setup_window.borrow_mut();

                        if !matches!(result, SetupResult::Finished) || setup_window.is_some() {
                            let setup = setup_window.get_or_insert_with(|| SetupWindow::new(manager, &obj));
                            setup.handle_setup_result(result);

                            if matches!(result, SetupResult::Finished) {
                                *setup_window = None;
                            }
                        }

                        None
                    }),
                );

                if let Err(e) = manager.init(&path).await {
                    let dialog = ErrorDialog::new(e, &obj);
                    dialog.present(&obj);
                }
            }));
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> =
                Lazy::new(|| vec![ParamSpecObject::builder::<Manager>("manager").build()]);
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
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
        fn close_request(&self) -> Propagation {
            if let Err(err) = self.obj().save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            self.parent_close_request()
        }
    }
    impl ApplicationWindowImpl for Window {}
    impl AdwWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}
