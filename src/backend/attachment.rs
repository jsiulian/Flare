use gdk::{prelude::TextureExt, Texture};
use gdk_pixbuf::{
    glib::{Bytes, Object, Priority},
    prelude::{FileExt, IOStreamExt, ObjectExt, OutputStreamExt},
};
use gio::{subclass::prelude::ObjectSubclassIsExt, File, FileCreateFlags};
use libsignal_service::{proto::AttachmentPointer, sender::AttachmentSpec};

use super::Manager;

gtk::glib::wrapper! {
    pub struct Attachment(ObjectSubclass<imp::Attachment>);
}

impl Attachment {
    pub fn from_file(file: File, manager: &Manager) -> Self {
        log::trace!("Trying to build a Attachment from a file");
        Object::new(&[
            ("manager", manager),
            ("file", &file),
            (
                "name",
                &file
                    .basename()
                    .and_then(|f| f.file_name().map(|s| s.to_string_lossy().into_owned())),
            ),
            ("image", &Texture::from_file(&file).ok()),
        ])
        .expect("Failed to create `Attachment`")
    }

    pub fn is_image(&self) -> bool {
        self.property::<bool>("is-image")
    }

    pub(super) async fn as_upload_attachment(&self) -> (AttachmentSpec, Vec<u8>) {
        let file = self.property::<File>("file");
        let image = self.property::<Option<Texture>>("image");
        let bytes = file
            .load_bytes_future()
            .await
            .expect("Failed to read the file")
            .0
            .to_vec();
        (
            AttachmentSpec {
                content_type: gio::content_type_guess(file.basename(), &bytes)
                    .0
                    .as_str()
                    .to_owned(),
                length: bytes.len(),
                file_name: file
                    .basename()
                    .and_then(|f| f.file_name().map(|s| s.to_string_lossy().to_string())),
                preview: None,
                voice_note: None,
                borderless: None,
                width: image.as_ref().and_then(|i| i.width().try_into().ok()),
                height: image.as_ref().and_then(|i| i.height().try_into().ok()),
                caption: None,
                blur_hash: None,
            },
            bytes,
        )
    }

    pub(super) async fn from_pointer(pointer: &AttachmentPointer, manager: &Manager) -> Self {
        crate::trace!("Trying to build a Attachment from a pointer",);
        log::trace!(
            "Attachment with content type: {}",
            pointer.content_type.as_ref().unwrap_or(&"None".to_string())
        );
        let mut image = None;
        let mut raw = None;
        let mut name = None;
        if let Some(pointer_name) = &pointer.file_name {
            name = Some(pointer_name.clone());
        }
        if let Ok(bytes) = manager.get_attachment(pointer).await {
            raw = Some(Bytes::from_owned(bytes));

            match &pointer.content_type {
                Some(t) if t.starts_with("image/") => {
                    log::trace!("Attachment is a image, converting to usable type");
                    image = Texture::from_bytes(raw.as_ref().expect("Raw bytes to be set")).ok();
                    if name.is_none() {
                        name = Some(format!("image.{}", &t[6..]));
                    }
                }
                Some(t) => log::trace!("Currently unhandles attachment type: {}", t),
                None => log::trace!("Attachment got no type"),
            }
        }
        let s: Self = Object::new(&[("manager", manager), ("image", &image), ("name", &name)])
            .expect("Failed to create `Attachment`");
        *s.imp().raw.borrow_mut() = raw;
        s
    }

    pub fn name(&self) -> Option<String> {
        self.property::<Option<String>>("name")
    }

    pub async fn save_to_file(&self, file: &File) -> Result<(), gtk::glib::error::Error> {
        log::trace!("Saving attachment to a file");
        let file_io = file
            .replace_readwrite_future(None, false, FileCreateFlags::NONE, Priority::default())
            .await?;
        let data = { self.imp().raw.borrow().clone() };
        if let Some(raw) = data {
            let stream = file_io.output_stream();
            stream.write_bytes_future(&raw, Priority::default()).await?;
        }
        Ok(())
    }
}

mod imp {
    use gdk::subclass::prelude::{ObjectImpl, ObjectSubclass};
    use gdk::Texture;
    use gdk_pixbuf::glib::{Bytes, ParamSpecBoolean, ParamSpecString};
    use gdk_pixbuf::prelude::ObjectExt;
    use gdk_pixbuf::{
        glib::{once_cell::sync::Lazy, ParamFlags, ParamSpec, ParamSpecObject, Value},
        prelude::{StaticType, ToValue},
    };
    use gio::File;
    use gtk::glib;
    use std::cell::RefCell;

    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Attachment {
        image: RefCell<Option<Texture>>,
        file: RefCell<Option<File>>,
        name: RefCell<Option<String>>,

        pub(super) raw: RefCell<Option<Bytes>>,

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
                    ParamSpecObject::new(
                        "file",
                        "file",
                        "file",
                        File::static_type(),
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                    ParamSpecString::new(
                        "name",
                        "name",
                        "name",
                        None,
                        ParamFlags::READWRITE.union(ParamFlags::CONSTRUCT_ONLY),
                    ),
                    ParamSpecBoolean::new(
                        "is-image",
                        "is-image",
                        "is-image",
                        false,
                        ParamFlags::READABLE,
                    ),
                    ParamSpecBoolean::new(
                        "is-file",
                        "is-file",
                        "is-file",
                        false,
                        ParamFlags::READABLE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "image" => self.image.borrow().as_ref().to_value(),
                "file" => self.file.borrow().as_ref().to_value(),
                "name" => self.name.borrow().as_ref().to_value(),
                "is-image" => obj
                    .property::<Option<Texture>>("image")
                    .is_some()
                    .to_value(),
                "is-file" => obj
                    .property::<Option<Texture>>("image")
                    .is_none()
                    .to_value(),
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
                "file" => {
                    let obj = value
                        .get::<Option<File>>()
                        .expect("Property `file` of `Message` has to be of type `File`");

                    self.file.replace(obj);
                }
                "name" => {
                    let obj = value
                        .get::<Option<String>>()
                        .expect("Property `name` of `Message` has to be of type `String`");

                    self.name.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
