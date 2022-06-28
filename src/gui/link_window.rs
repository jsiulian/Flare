use gdk_pixbuf::prelude::IsA;
use gtk::glib::Object;

use crate::backend::Manager;

gtk::glib::wrapper! {
    pub struct LinkWindow(ObjectSubclass<imp::LinkWindow>)
        @extends gtk::Dialog, gtk::Window, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl LinkWindow {
    pub fn new(url: String, manager: Manager, parent: &impl IsA<gtk::Window>) -> Self {
        log::trace!("Initializing link window");
        Object::new(&[
            ("url", &url),
            ("manager", &manager),
            ("transient-for", &parent),
        ])
        .expect("Failed to create LinkWindow")
    }
}

pub mod imp {
    use std::cell::RefCell;

    use gdk_pixbuf::glib::clone;
    use gdk_pixbuf::glib::once_cell::sync::Lazy;
    use gdk_pixbuf::glib::Bytes;
    use gdk_pixbuf::glib::ParamFlags;
    use gdk_pixbuf::glib::ParamSpec;
    use gdk_pixbuf::glib::ParamSpecObject;
    use gdk_pixbuf::glib::ParamSpecString;
    use gdk_pixbuf::glib::Value;
    use gdk_pixbuf::Pixbuf;
    use gio::MemoryInputStream;
    use glib::subclass::InitializingObject;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;
    use libadwaita::subclass::prelude::AdwApplicationWindowImpl;
    use libadwaita::subclass::prelude::AdwWindowImpl;

    use crate::backend::Manager;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/link_window.ui")]
    pub struct LinkWindow {
        #[template_child]
        qr_image: TemplateChild<gtk::Picture>,

        url: RefCell<Option<String>>,
        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LinkWindow {
        const NAME: &'static str = "FlLinkWindow";
        type Type = super::LinkWindow;
        type ParentType = gtk::Dialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LinkWindow {
        fn constructed(&self, obj: &Self::Type) {
            log::trace!("Constructed LinkWindow");
            self.parent_constructed(obj);
            // TODO: Cancel manager?
            obj.connect_response(|dialog, _| dialog.close());
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecString::new(
                        "url",
                        "url",
                        "url",
                        None,
                        ParamFlags::READWRITE | ParamFlags::CONSTRUCT_ONLY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "url" => self.url.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `LinkWindow` has to be of type `Manager`");

                    if let Some(man) = man.as_ref() {
                        man.connect_local(
                            "link-finish",
                            false,
                            clone!(@strong obj => move |_| {obj.emit_close(); None}),
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
                            &url,
                            qrcode_generator::QrCodeEcc::Low,
                            1024,
                        )
                        .expect("Failed to generate QR code");
                        let bytes_glib = Bytes::from_owned(bytes_vec);
                        let stream = MemoryInputStream::from_bytes(&bytes_glib);
                        let pixbuf = Pixbuf::from_stream(&stream, None::<&gio::Cancellable>)
                            .expect("Failed to generate Pixbuf from stream");
                        self.qr_image.set_pixbuf(Some(&pixbuf));
                    }

                    self.url.replace(url);
                }
                _ => unimplemented!(),
            }
        }
    }

    impl DialogImpl for LinkWindow {}
    impl WidgetImpl for LinkWindow {}
    impl WindowImpl for LinkWindow {}
    impl ApplicationWindowImpl for LinkWindow {}
    impl AdwWindowImpl for LinkWindow {}
    impl AdwApplicationWindowImpl for LinkWindow {}
}
