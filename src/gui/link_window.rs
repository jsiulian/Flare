use crate::backend::Manager;
use glib::{prelude::IsA, Object, ObjectExt};
use gtk::glib;

glib::wrapper! {
    pub struct LinkWindow(ObjectSubclass<imp::LinkWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl LinkWindow {
    pub fn new(url: String, manager: Manager, parent: &impl IsA<gtk::Window>) -> Self {
        log::trace!("Initializing link window");
        Object::builder::<Self>()
            .property("url", &url)
            .property("manager", &manager)
            .property("transient-for", parent)
            .build()
    }

    pub fn url(&self) -> String {
        self.property("url")
    }
}

pub mod imp {
    use adw::subclass::prelude::*;
    use adw::Toast;
    use gdk::gdk_pixbuf::Pixbuf;
    use gettextrs::gettext;
    use gio::MemoryInputStream;
    use glib::{
        clone, once_cell::sync::Lazy, subclass::InitializingObject, Bytes, ParamSpec,
        ParamSpecObject, ParamSpecString, Value,
    };
    use gtk::{gdk, gio, glib};
    use gtk::{prelude::*, CompositeTemplate};
    use std::cell::RefCell;

    use crate::backend::Manager;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/link_window.ui")]
    pub struct LinkWindow {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) content: TemplateChild<adw::NavigationView>,
        #[template_child]
        page_manual: TemplateChild<adw::NavigationPage>,
        #[template_child]
        qr_image: TemplateChild<gtk::Picture>,

        url: RefCell<Option<String>>,
        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl LinkWindow {
        #[template_callback]
        fn handle_clipboard(&self, _: gtk::Button) {
            let obj = self.obj();
            let clipboard = obj.clipboard();
            clipboard.set_text(&obj.url());

            let toast = Toast::new(&gettext("Copied to clipboard"));
            obj.imp().toast_overlay.add_toast(toast);
        }
        #[template_callback]
        fn previous(&self) {
            let obj = self.obj();
            obj.imp().content.pop();
        }
        #[template_callback]
        fn forward(&self) {
            let obj = self.obj();
            obj.imp().content.push(&self.page_manual.get());
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LinkWindow {
        const NAME: &'static str = "FlLinkWindow";
        type Type = super::LinkWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LinkWindow {
        fn constructed(&self) {
            log::trace!("Constructed LinkWindow");
            self.parent_constructed();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecString::builder("url").construct_only().build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "url" => self.url.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            let instance = self.obj();
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `LinkWindow` has to be of type `Manager`");

                    if let Some(man) = man.as_ref() {
                        man.connect_local(
                            "link-finish",
                            false,
                            clone!(@strong instance as obj => move |_| {obj.close(); None}),
                        );
                    }

                    self.manager.replace(man);
                }
                "url" => {
                    let url = value
                        .get::<Option<String>>()
                        .expect("Property `url` of `LinkWindow` has to be of type `String`");

                    if let Some(url) = url.as_ref() {
                        let bytes_vec = qrcode_generator::to_png_to_vec(
                            url,
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
                    }

                    self.url.replace(url);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for LinkWindow {}
    impl WindowImpl for LinkWindow {}
    impl AdwWindowImpl for LinkWindow {}
}
