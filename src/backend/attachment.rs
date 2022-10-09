use std::path::PathBuf;

use gdk::{prelude::TextureExt, Texture};
use gio::{prelude::*, subclass::prelude::ObjectSubclassIsExt, Cancellable, File, FileCreateFlags};
use glib::{Bytes, Object, Priority};
use gtk::{MediaFile, MediaStream};
use libsignal_service::{proto::AttachmentPointer, sender::AttachmentSpec};

use super::Manager;

gtk::glib::wrapper! {
    pub struct Attachment(ObjectSubclass<imp::Attachment>);
}

impl Attachment {
    pub fn from_file(file: File, manager: &Manager) -> Self {
        log::trace!("Trying to build a Attachment from a file");
        let mime = gio::content_type_guess(file.basename(), &[])
            .0
            .as_str()
            .to_owned();
        let mut image = None;
        if mime.starts_with("image/") {
            image = Texture::from_file(&file).ok();
        }
        let mut video = None;
        if mime.starts_with("video/") {
            video = Some(MediaFile::for_file(&file))
        }
        Object::new(&[
            ("manager", manager),
            ("file", &file),
            (
                "name",
                &file
                    .basename()
                    .and_then(|f| f.file_name().map(|s| s.to_string_lossy().into_owned())),
            ),
            ("image", &image),
            ("video", &video),
            ("loaded", &true),
            ("content-type", &mime),
        ])
        .expect("Failed to create `Attachment`")
    }

    pub fn is_image(&self) -> bool {
        self.property::<bool>("is-image")
    }

    pub fn is_video(&self) -> bool {
        self.property::<bool>("is-video")
    }

    pub fn is_file(&self) -> bool {
        self.property::<bool>("is-file")
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
        let s: Self = Object::new(&[
            ("manager", manager),
            ("image", &None::<Texture>),
            ("name", &None::<String>),
            ("video", &None::<MediaStream>),
            ("loaded", &false),
            ("content-type", &pointer.content_type.as_ref()),
        ])
        .expect("Failed to create `Attachment`");
        *s.imp().pointer.borrow_mut() = Some(pointer.clone());
        s
    }

    pub async fn load(&self) {
        let pointer_opt = { self.imp().pointer.borrow().clone() };
        if pointer_opt.is_none() {
            return;
        }
        let pointer = pointer_opt.as_ref().unwrap();
        let manager = self.property::<Manager>("manager");

        let mut image = None;
        let mut video = None;
        let mut raw = None;
        let mut name = None;
        let mut file = None;

        // Populate name if set.
        if let Some(pointer_name) = &pointer.file_name {
            name = Some(pointer_name.clone());
        }

        if let Ok(bytes) = manager.get_attachment(pointer).await {
            raw = Some(Bytes::from_owned(bytes));

            // TODO: Crashes, see <https://gitlab.gnome.org/GNOME/gtk/-/issues/4062>
            // let stream =
            //     MemoryInputStream::from_bytes(raw.as_ref().expect("Raw bytes to be set"));
            // TODO: Async
            // Write to file.
            let tmp = File::new_tmp(None::<PathBuf>).ok();
            if let Some((tmp_file, tmp_file_stream)) = tmp {
                let tmp_out = tmp_file_stream.output_stream();
                let _ = tmp_out.write_bytes(
                    raw.as_ref().expect("Raw bytes to be set"),
                    Cancellable::NONE,
                );
                let _ = tmp_out.flush(Cancellable::NONE);
                file = Some(tmp_file);
            }

            match &pointer.content_type {
                Some(t) if t.starts_with("image/") => {
                    log::trace!("Attachment is a image, converting to usable type");
                    image = Texture::from_bytes(raw.as_ref().expect("Raw bytes to be set")).ok();
                    if name.is_none() {
                        name = Some(format!("image.{}", &t[6..]));
                    }
                }
                Some(t) if t.starts_with("video/") => {
                    log::trace!("Attachment is a video, converting to usable type");
                    if let Some(tmp_file) = file.as_ref() {
                        video = Some(MediaFile::for_file(tmp_file));
                        if name.is_none() {
                            name = Some(format!("video.{}", &t[6..]));
                        }
                    }
                }
                Some(t) => log::trace!("Currently unhandles attachment type: {}", t),
                None => log::trace!("Attachment got no type"),
            }
        }

        self.set_property("file", file);
        self.set_property("image", image);
        self.set_property("video", video);
        self.set_property("name", name);
        self.notify("is-image");
        self.notify("is-video");
        self.notify("is-file");
        *self.imp().raw.borrow_mut() = raw;
        self.set_property("loaded", true);
    }

    pub fn name(&self) -> Option<String> {
        self.property::<Option<String>>("name")
    }

    pub fn open_file(&self) -> Option<std::fs::File> {
        self.property::<Option<File>>("file").and_then(|f| f.path()).and_then(|p| std::fs::File::open(p).ok())
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
    use std::cell::{Cell, RefCell};

    use gdk::prelude::*;
    use gdk::{subclass::prelude::*, Texture};
    use gio::File;
    use glib::{
        once_cell::sync::Lazy, Bytes, ParamFlags, ParamSpec, ParamSpecBoolean, ParamSpecObject,
        ParamSpecString, Value,
    };
    use gtk::MediaStream;
    use libsignal_service::proto::AttachmentPointer;

    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Attachment {
        image: RefCell<Option<Texture>>,
        video: RefCell<Option<MediaStream>>,
        file: RefCell<Option<File>>,
        name: RefCell<Option<String>>,

        content_type: RefCell<Option<String>>,

        loaded: Cell<bool>,

        pub(super) pointer: RefCell<Option<AttachmentPointer>>,
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
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "video",
                        "video",
                        "video",
                        MediaStream::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecObject::new(
                        "file",
                        "file",
                        "file",
                        File::static_type(),
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecString::new("name", "name", "name", None, ParamFlags::READWRITE),
                    ParamSpecString::new(
                        "content-type",
                        "content-type",
                        "content-type",
                        None,
                        ParamFlags::READWRITE,
                    ),
                    ParamSpecBoolean::new(
                        "is-image",
                        "is-image",
                        "is-image",
                        false,
                        ParamFlags::READABLE,
                    ),
                    ParamSpecBoolean::new(
                        "is-video",
                        "is-video",
                        "is-video",
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
                    ParamSpecBoolean::new(
                        "loaded",
                        "loaded",
                        "loaded",
                        false,
                        ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "image" => self.image.borrow().as_ref().to_value(),
                "video" => self.video.borrow().as_ref().to_value(),
                "file" => self.file.borrow().as_ref().to_value(),
                "name" => self.name.borrow().as_ref().to_value(),
                "is-image" => obj
                    .property::<Option<String>>("content-type")
                    .map(|s| s.starts_with("image/"))
                    .unwrap_or_default()
                    .to_value(),
                "content-type" => self.content_type.borrow().as_ref().to_value(),
                "is-video" => obj
                    .property::<Option<String>>("content-type")
                    .map(|s| s.starts_with("video/"))
                    .unwrap_or_default()
                    .to_value(),
                "is-file" => (!(obj.property::<bool>("is-image")
                    || obj.property::<bool>("is-video")))
                .to_value(),
                "loaded" => self.loaded.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let obj = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Attachment` has to be of type `Manager`");

                    self.manager.replace(obj);
                }
                "image" => {
                    let obj = value
                        .get::<Option<Texture>>()
                        .expect("Property `image` of `Attachment` has to be of type `Texture`");

                    self.image.replace(obj);
                }
                "video" => {
                    let obj = value
                        .get::<Option<MediaStream>>()
                        .expect("Property `video` of `Attachment` has to be of type `MediaStream`");

                    self.video.replace(obj);
                }
                "file" => {
                    let obj = value
                        .get::<Option<File>>()
                        .expect("Property `file` of `Attachment` has to be of type `File`");

                    self.file.replace(obj);
                }
                "name" => {
                    let obj = value
                        .get::<Option<String>>()
                        .expect("Property `name` of `Attachment` has to be of type `String`");

                    self.name.replace(obj);
                }
                "content-type" => {
                    let obj = value.get::<Option<String>>().expect(
                        "Property `content-type` of `Attachment` has to be of type `String`",
                    );

                    self.content_type.replace(obj);
                }
                "loaded" => {
                    let obj = value
                        .get::<bool>()
                        .expect("Property `loaded` of `Attachment` has to be of type `bool`");

                    self.loaded.replace(obj);
                }
                _ => unimplemented!(),
            }
        }
    }
}
