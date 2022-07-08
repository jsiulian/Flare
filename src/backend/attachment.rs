use gdk::Texture;
use gdk_pixbuf::glib::{Bytes, Object};
use libsignal_service::proto::AttachmentPointer;

use super::Manager;

gtk::glib::wrapper! {
    pub struct Attachment(ObjectSubclass<imp::Attachment>);
}

impl Attachment {
    pub(super) async fn from_pointer(pointer: &AttachmentPointer, manager: &Manager) -> Self {
        log::trace!("Trying to build a Attachment from a pointer");
        log::trace!(
            "Attachment with content type: {}",
            pointer.content_type.as_ref().unwrap_or(&"None".to_string())
        );
        let mut image = None;
        if let Ok(bytes) = manager.internal().get_attachment(pointer).await {
            match &pointer.content_type {
                Some(t) if t.starts_with("image/") => {
                    log::trace!("Attachment is a image, converting to usable type");
                    let glib_bytes = Bytes::from_owned(bytes);
                    image = Texture::from_bytes(&glib_bytes).ok();
                }
                Some(t) => log::trace!("Currently unhandles attachment type: {}", t),
                None => log::trace!("Attachment got no type"),
            }
        }
        Object::new(&[("manager", manager), ("image", &image)])
            .expect("Failed to create `Attachment`")
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk::Texture;
    use gdk_pixbuf::{
        glib::{once_cell::sync::Lazy, ParamFlags, ParamSpec, ParamSpecObject, Value},
        prelude::{StaticType, ToValue},
    };
    use gtk::glib;
    use std::cell::RefCell;

    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Attachment {
        image: RefCell<Option<Texture>>,

        manager: RefCell<Option<Manager>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Attachment {
        const NAME: &'static str = "FlAttachmentObject";
        type Type = super::Attachment;
    }

    impl ObjectImpl for Attachment {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::new(
                        "manager",
                        "manager",
                        "manager",
                        Manager::static_type(),
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                    ParamSpecObject::new(
                        "image",
                        "image",
                        "image",
                        Texture::static_type(),
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "image" => self.image.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Message` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                "image" => {
                    let obj = value
                        .get::<Option<Texture>>()
                        .expect("Property `image` of `Message` has to be of type `Texture`");

                    self.image.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
