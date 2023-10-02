use crate::backend::Manager;
use glib::{prelude::IsA, Object, ObjectExt};
use gtk::glib;

glib::wrapper! {
    pub struct SetupWindow(ObjectSubclass<imp::SetupWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl SetupWindow {
    pub fn new(manager: Manager, parent: &impl IsA<gtk::Window>) -> Self {
        log::trace!("Initializing link window");
        Object::builder::<Self>()
            .property("manager", &manager)
            .property("transient-for", parent)
            .build()
    }

    fn manager(&self) -> Manager {
        self.property("manager")
    }
}

pub mod imp {
    use adw::subclass::prelude::*;
    use adw::Toast;
    use futures::channel::oneshot::Sender;
    use gdk::gdk_pixbuf::Pixbuf;
    use gdk::glib::BoxedAnyObject;
    use gettextrs::gettext;
    use gio::MemoryInputStream;
    use glib::{
        clone, once_cell::sync::Lazy, subclass::InitializingObject, Bytes, ParamSpec,
        ParamSpecObject, Value,
    };
    use gtk::{gdk, gio, glib};
    use gtk::{prelude::*, CompositeTemplate};
    use presage::prelude::PhoneNumber;
    use std::cell::{RefCell};
    use std::str::FromStr;

    use crate::backend::{Manager, SetupResult, SetupDecision};

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

        manager: RefCell<Option<Manager>>,

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
                clipboard.set_text(&url);

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
            self.entry_device_name.set_text(&self.obj().manager().settings().string("link-device-name"));
            self.content.push(&self.page_decision_link.get());
        }

        #[template_callback]
        fn handle_decision_to_decision_primary(&self) {
            self.content.push(&self.page_decision_primary.get());
        }

        #[template_callback]
        fn handle_link_confirm(&self) {
            if let Some(callback) = self.decision_callback.take() {
                // TODO: Configurable server.
                // TODO: Disallow empty device name.
                callback.send(SetupDecision::Link(libsignal_service::configuration::SignalServers::Production, self.entry_device_name.text().to_string())).expect("Failed to send setup decision");
                // TODO: Maybe display spinner afterwards?
            }
        }

        #[template_callback]
        fn handle_primary_confirm(&self) {
            if let Some(callback) = self.confirm_callback.take() {
                // TODO: Configurable server.
                // TODO: Disallow empty device name.
                callback.send(self.entry_confirm.text().to_string()).expect("Failed to send setup decision");
                // TODO: Maybe display spinner afterwards?
            }
        }

        #[template_callback]
        fn handle_primary_decision_confirm(&self) {
            let mut captcha = self.entry_captch.text().to_string();
            if let Some(c) = captcha.strip_prefix("signalcaptcha://") {
                log::trace!("Captcha is the full link. Remove unneeded thigs.");
                captcha = c.to_owned();
            }

            if let Ok(phone) = PhoneNumber::from_str(&self.entry_phone_number.text().to_string()) {
                // TODO: Error on phone number parsing
                if let Some(callback) = self.decision_callback.take() {
                    // TODO: Configurable server.

                    callback.send(SetupDecision::Register(libsignal_service::configuration::SignalServers::Staging, phone, captcha)).expect("Failed to send setup decision");
                    // TODO: Maybe display spinner afterwards?
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

        fn setup_manager(&self) {
            let obj = self.obj();
            let manager = obj.manager();

            manager.connect_local(
                "setup-result",
                false,
                clone!(@strong obj => move |r| {
                    // r[0] is the manager
                    let result = r[1].get::<BoxedAnyObject>().expect("Setup-Result to be BoxedAnyObject");
                    let result: &mut SetupResult = &mut *result.borrow_mut();

                    obj.imp().handle_setup_result(result);
                   
                    None
                }),
            );
        }

        fn handle_setup_result(&self, result: &mut SetupResult) {
            let obj = self.obj();
            match result {
                SetupResult::Pending(callback) => {
                    obj.present();
                    self.decision_callback.replace(callback.take());
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
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SetupWindow {
        const NAME: &'static str = "FlSetupWindow";
        type Type = super::SetupWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SetupWindow {
        fn constructed(&self) {
            log::trace!("Constructed SetupWindow");
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![ParamSpecObject::builder::<Manager>("manager")
                    .construct_only()
                    .build()]
            });
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
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `SetupWindow` has to be of type `Manager`");

                    self.manager.replace(man);

                    self.setup_manager();
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for SetupWindow {}
    impl WindowImpl for SetupWindow {}
    impl AdwWindowImpl for SetupWindow {}
}
