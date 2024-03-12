use crate::backend::{Manager, Server, SetupResult};
use gdk::glib::subclass::types::ObjectSubclassIsExt;
use glib::{prelude::ObjectExt, Object};
use gtk::glib;
use libsignal_service::configuration::SignalServers;

use super::Window;

glib::wrapper! {
    pub struct SetupWindow(ObjectSubclass<imp::SetupWindow>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl SetupWindow {
    pub fn new(manager: Manager, parent: &Window) -> Self {
        log::trace!("Initializing setup window");
        Object::builder::<Self>()
            .property("manager", &manager)
            .property("window", parent)
            .build()
    }

    pub fn handle_setup_result(&self, result: &mut SetupResult) {
        self.imp().handle_setup_result(result)
    }

    fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub fn window(&self) -> Window {
        self.property("window")
    }
}

fn servers() -> Vec<Server> {
    vec![
        Server::new(
            gettextrs::pgettext("Signal Server", "Production"),
            SignalServers::Production,
        ),
        Server::new(
            gettextrs::pgettext("Signal Server", "Staging"),
            SignalServers::Staging,
        ),
    ]
}

pub mod imp {
    use adw::prelude::AdwDialogExt;
    use adw::prelude::ComboRowExt;
    use adw::subclass::dialog::AdwDialogImplExt;
    use adw::subclass::prelude::*;
    use adw::Toast;
    use futures::channel::oneshot::Sender;
    use gdk::gdk_pixbuf::Pixbuf;
    use gdk::gio::ListStore;
    use gettextrs::gettext;
    use gio::MemoryInputStream;
    use glib::{subclass::InitializingObject, Bytes, ParamSpec, ParamSpecObject, Value};
    use gtk::{gdk, gio, glib, PropertyExpression};
    use gtk::{prelude::*, CompositeTemplate};
    use libsignal_service::configuration::SignalServers;
    use libsignal_service::prelude::phonenumber::PhoneNumber;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;
    use std::str::FromStr;

    use crate::backend::{Manager, Server, SetupDecision, SetupResult};
    use crate::gui::utility::Utility;
    use crate::gui::Window;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/setup_window.ui")]
    pub struct SetupWindow {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) content: TemplateChild<adw::NavigationView>,

        #[template_child]
        page_welcome: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_decision: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_decision_link: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_decision_primary: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_link_qr: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_link_manual: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_primary_confirm: TemplateChild<adw::NavigationPage>,
        #[template_child]
        page_finished: TemplateChild<adw::NavigationPage>,

        #[template_child]
        entry_device_name: TemplateChild<adw::EntryRow>,
        #[template_child]
        entry_phone_number: TemplateChild<adw::EntryRow>,
        #[template_child]
        entry_captch: TemplateChild<adw::EntryRow>,
        #[template_child]
        entry_confirm: TemplateChild<adw::EntryRow>,

        #[template_child]
        qr_image: TemplateChild<gtk::Picture>,

        #[template_child]
        dropdown_primary_server: TemplateChild<adw::ComboRow>,
        #[template_child]
        dropdown_link_server: TemplateChild<adw::ComboRow>,

        manager: RefCell<Option<Manager>>,

        window: RefCell<Option<Window>>,

        decision_callback: RefCell<Option<Sender<SetupDecision>>>,
        confirm_callback: RefCell<Option<Sender<String>>>,

        url: RefCell<Option<String>>,
    }

    #[gtk::template_callbacks]
    impl SetupWindow {
        #[template_callback]
        fn handle_clipboard(&self, _: gtk::Button) {
            if let Some(url) = self.url.borrow().as_ref() {
                let obj = self.obj();
                let clipboard = obj.clipboard();
                clipboard.set_text(url);

                let toast = Toast::new(&gettext("Copied to clipboard"));
                obj.imp().toast_overlay.add_toast(toast);
            }
        }

        #[template_callback]
        fn previous(&self) {
            let obj = self.obj();
            obj.imp().content.pop();
        }

        #[template_callback]
        fn handle_welcome_to_decision(&self) {
            self.content.push(&self.page_decision.get());
        }

        #[template_callback]
        fn handle_decision_to_decision_link(&self) {
            self.entry_device_name
                .set_text(&self.obj().manager().settings().string("link-device-name"));
            self.content.push(&self.page_decision_link.get());
        }

        #[template_callback]
        fn handle_decision_to_decision_primary(&self) {
            self.content.push(&self.page_decision_primary.get());
        }

        #[template_callback]
        fn handle_link_confirm(&self) {
            // Should always be given due to UI only being on that page after setup decision was asked for.
            if let Some(callback) = self.decision_callback.take() {
                // Device name will not be empty as UI button only sensitive when the device name is non-empty.
                let device_name = self.entry_device_name.text().to_string();
                let _ = self
                    .obj()
                    .manager()
                    .settings()
                    .set_string("link-device-name", &device_name);
                callback
                    .send(SetupDecision::Link(
                        self.dropdown_link_server
                            .selected_item()
                            .and_dynamic_cast::<Server>()
                            .map(|s| s.server())
                            .unwrap_or(SignalServers::Production),
                        device_name,
                    ))
                    .expect("Failed to send setup decision");
                // XXX: Maybe display spinner afterwards?
            }
        }

        #[template_callback]
        fn handle_primary_confirm(&self) {
            if let Some(callback) = self.confirm_callback.take() {
                callback
                    .send(self.entry_confirm.text().to_string())
                    .expect("Failed to send setup decision");
                // XXX: Maybe display spinner afterwards?
            }
        }

        #[template_callback]
        fn handle_primary_decision_confirm(&self) {
            let mut captcha = self.entry_captch.text().to_string();
            if let Some(c) = captcha.strip_prefix("signalcaptcha://") {
                log::trace!("Captcha is the full link. Remove unneeded thigs.");
                captcha = c.to_owned();
            }

            // Should always succeed due to UI button only being sensitive when the text is a phone number.
            if let Ok(phone) = PhoneNumber::from_str(self.entry_phone_number.text().as_ref()) {
                // Should always be given due to UI only being on that page after setup decision was asked for.
                if let Some(callback) = self.decision_callback.take() {
                    callback
                        .send(SetupDecision::Register(
                            self.dropdown_primary_server
                                .selected_item()
                                .and_dynamic_cast::<Server>()
                                .map(|s| s.server())
                                .unwrap_or(SignalServers::Production),
                            phone,
                            captcha,
                        ))
                        .expect("Failed to send setup decision");
                    // XXX: Maybe display spinner afterwards?
                }
            }
        }

        #[template_callback]
        fn handle_link_qr_to_link_manual(&self) {
            self.content.push(&self.page_link_manual.get());
        }

        #[template_callback]
        fn handle_finished_close(&self) {
            self.obj().close();
        }

        pub(super) fn handle_setup_result(&self, result: &mut SetupResult) {
            let obj = self.obj();
            match result {
                SetupResult::Pending(callback) => {
                    obj.present(&obj.window());
                    self.decision_callback.replace(callback.take());

                    let win = obj.window();

                    win.destroy_if_invisible();
                }
                SetupResult::DisplayLinkQR(url) => {
                    let url = url.to_string();
                    let bytes_vec = qrcode_generator::to_png_to_vec(
                        url.clone(),
                        qrcode_generator::QrCodeEcc::Low,
                        200,
                    )
                    .expect("Failed to generate QR code");
                    let bytes_glib = Bytes::from_owned(bytes_vec);
                    let stream = MemoryInputStream::from_bytes(&bytes_glib);
                    let pixbuf = Pixbuf::from_stream(&stream, None::<&gio::Cancellable>)
                        .expect("Failed to generate Pixbuf from stream");
                    self.qr_image
                        .set_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));

                    self.url.replace(Some(url));

                    self.content.push(&self.page_link_qr.get());
                }
                SetupResult::Confirm(callback) => {
                    self.confirm_callback.replace(callback.take());
                    self.content.push(&self.page_primary_confirm.get());
                }
                SetupResult::Finished => {
                    self.content.push(&self.page_finished.get());
                }
            }
        }

        fn setup_servers_dropdown(&self) {
            let model = ListStore::from_iter(super::servers());
            let expression = PropertyExpression::new(
                Server::static_type(),
                None::<PropertyExpression>,
                "display",
            );

            self.dropdown_primary_server
                .set_expression(Some(&expression));
            self.dropdown_link_server.set_expression(Some(&expression));

            self.dropdown_primary_server.set_model(Some(&model));
            self.dropdown_link_server.set_model(Some(&model));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SetupWindow {
        const NAME: &'static str = "FlSetupWindow";
        type Type = super::SetupWindow;
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

    impl ObjectImpl for SetupWindow {
        fn constructed(&self) {
            log::trace!("Constructed SetupWindow");
            self.setup_servers_dropdown();
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Window>("window")
                        .construct_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "window" => self.window.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `SetupWindow` has to be of type `Manager`");

                    self.manager.replace(man);
                }
                "window" => {
                    let win = value
                        .get::<Option<Window>>()
                        .expect("Property `window` of `SetupWindow` has to be of type `Window`");

                    self.window.replace(win);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for SetupWindow {}
    impl AdwDialogImpl for SetupWindow {
        fn closed(&self) {
            // If it is closed while not being finished, close the parent window
            if self.content.visible_page() != Some(self.page_finished.get()) {
                self.obj().window().destroy();
            }

            self.parent_closed()
        }
    }
}
