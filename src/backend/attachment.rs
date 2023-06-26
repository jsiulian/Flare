use std::path::PathBuf;

use gdk::{prelude::TextureExt, Texture};
use gio::{prelude::*, subclass::prelude::ObjectSubclassIsExt, Cancellable, File, FileCreateFlags};
use glib::{Bytes, Object, Priority};
use gtk::{gdk, gio, glib};
use gtk::{MediaFile, MediaStream};
use presage::prelude::{content::AttachmentPointer, AttachmentSpec};

use super::Manager;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, glib::Enum, Default)]
#[repr(u32)]
#[enum_type(name = "FlAttachmentType")]
pub enum AttachmentType {
    Image,
    Video,
    Audio,
    #[default]
    File,
}

impl AttachmentType {
    fn from_content_type<S: AsRef<str>>(content: S) -> Self {
        let content = content.as_ref();
        if content.starts_with("image/") {
            Self::Image
        } else if content.starts_with("video/") {
            Self::Video
        } else if content.starts_with("audio/") {
            Self::Audio
        } else {
            Self::File
        }
    }
}

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
        let mut audio = None;
        if mime.starts_with("audio/") {
            audio = Some(MediaFile::for_file(&file))
        }
        Object::builder::<Self>()
            .property("manager", manager)
            .property("file", &file)
            .property(
                "name",
                &file
                    .basename()
                    .and_then(|f| f.file_name().map(|s| s.to_string_lossy().into_owned())),
            )
            .property("image", &image)
            .property("video", &video)
            .property("audio", &audio)
            .property("loaded", true)
            .property("content-type", &mime)
            .build()
    }

    pub fn from_texture(texture: Texture, manager: &Manager) -> Self {
        let (file, tmp_file_stream) =
            File::new_tmp(None::<PathBuf>).expect("Failed to create temporary file");
        let tmp_out = tmp_file_stream.output_stream();
        let _ = tmp_out.write_bytes(&texture.save_to_png_bytes(), Cancellable::NONE);

        Object::builder::<Self>()
            .property("manager", manager)
            .property("file", &file)
            .property("name", "image.png")
            .property("image", &texture)
            .property("loaded", true)
            .property("content-type", "image/png")
            .build()
    }

    pub fn manager(&self) -> Manager {
        self.property("manager")
    }

    pub fn content_type(&self) -> Option<String> {
        self.property("content-type")
    }

    pub fn attachment_type(&self) -> AttachmentType {
        self.property::<AttachmentType>("type")
    }

    pub fn is_image(&self) -> bool {
        self.attachment_type() == AttachmentType::Image
    }

    pub fn is_video(&self) -> bool {
        self.attachment_type() == AttachmentType::Video
    }

    pub fn is_audio(&self) -> bool {
        self.attachment_type() == AttachmentType::Audio
    }

    pub fn is_file(&self) -> bool {
        self.attachment_type() == AttachmentType::File
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
                content_type: self.content_type().unwrap_or_else(|| {
                    gio::content_type_guess(file.basename(), &bytes)
                        .0
                        .as_str()
                        .to_owned()
                }),
                length: bytes.len(),
                file_name: self.name().or_else(|| {
                    file.basename()
                        .and_then(|f| f.file_name().map(|s| s.to_string_lossy().to_string()))
                }),
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
        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .property("image", &None::<Texture>)
            .property("name", &None::<String>)
            .property("video", &None::<MediaStream>)
            .property("audio", &None::<MediaStream>)
            .property("loaded", false)
            .property("content-type", pointer.content_type.as_ref())
            .build();
        *s.imp().pointer.borrow_mut() = Some(pointer.clone());
        s
    }

    pub async fn load(&self) {
        let pointer_opt = { self.imp().pointer.borrow().clone() };
        if pointer_opt.is_none() {
            return;
        }
        let pointer = pointer_opt.as_ref().unwrap();
        let manager = self.manager();

        let mut image = None;
        let mut video = None;
        let mut audio = None;
        let mut raw = None;
        let mut name = None;
        let mut file = None;

        // Populate name if set.
        if let Some(pointer_name) = &pointer.file_name {
            name = Some(pointer_name.clone());
        }

        if name.is_none() {
            match &pointer.content_type {
                Some(t) if t.starts_with("image/") => name = Some(format!("image.{}", &t[6..])),
                Some(t) if t.starts_with("video/") => name = Some(format!("video.{}", &t[6..])),
                Some(t) if t.starts_with("audio/") => name = Some(format!("audio.{}", &t[6..])),
                _ => {}
            }
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
                if let Some(name) = name.as_ref() {
                    let renamed_file = tmp_file.set_display_name(
                        &format!(
                            "{}.{}",
                            tmp_file
                                .basename()
                                .map(|b| b.display().to_string())
                                .unwrap_or_default(),
                            name,
                        ),
                        None::<&gio::Cancellable>,
                    );
                    if let Ok(renamed_file) = renamed_file {
                        file = Some(renamed_file);
                    } else {
                        file = Some(tmp_file);
                    }
                } else {
                    file = Some(tmp_file);
                }
            }

            match &pointer.content_type {
                Some(t) if t.starts_with("image/") => {
                    log::trace!("Attachment is a image, converting to usable type");
                    image = Texture::from_bytes(raw.as_ref().expect("Raw bytes to be set")).ok();
                }
                Some(t) if t.starts_with("video/") => {
                    log::trace!("Attachment is a video, converting to usable type");
                    if let Some(tmp_file) = file.as_ref() {
                        video = Some(MediaFile::for_file(tmp_file));
                    }
                }
                Some(t) if t.starts_with("audio/") => {
                    log::trace!("Attachment is a voice message , converting to usable type");
                    if let Some(tmp_file) = file.as_ref() {
                        audio = Some(MediaFile::for_file(tmp_file));
                    }
                }
                Some(t) => log::trace!("Currently unhandles attachment type: {}", t),
                None => log::trace!("Attachment got no type"),
            }
        }

        self.set_property("file", file);
        self.set_property("image", image);
        self.set_property("video", video);
        self.set_property("audio", audio);
        self.set_property("name", name);
        self.notify("type");
        self.notify("is-image");
        self.notify("is-video");
        self.notify("is-audio");
        self.notify("is-file");
        *self.imp().raw.borrow_mut() = raw;
        self.set_property("loaded", true);
    }

    pub fn name(&self) -> Option<String> {
        self.property::<Option<String>>("name")
    }

    pub fn uri(&self) -> Option<glib::GString> {
        self.property::<Option<File>>("file").map(|f| f.uri())
    }

    pub fn open_file(&self) -> Option<std::fs::File> {
        self.property::<Option<File>>("file")
            .and_then(|f| f.path())
            .and_then(|p| std::fs::File::open(p).ok())
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
    use glib::ParamSpecEnum;
    use glib::{
        once_cell::sync::Lazy, Bytes, ParamSpec, ParamSpecBoolean, ParamSpecObject,
        ParamSpecString, Value,
    };
    use gtk::MediaStream;
    use gtk::{gdk, gio, glib};
    use presage::prelude::content::AttachmentPointer;

    use crate::backend::attachment::AttachmentType;
    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Attachment {
        image: RefCell<Option<Texture>>,
        video: RefCell<Option<MediaStream>>,
        audio: RefCell<Option<MediaStream>>,
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
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<Texture>("image").build(),
                    ParamSpecObject::builder::<MediaStream>("video").build(),
                    ParamSpecObject::builder::<MediaStream>("audio").build(),
                    ParamSpecObject::builder::<File>("file").build(),
                    ParamSpecEnum::builder::<AttachmentType>("type")
                        .read_only()
                        .default_value(AttachmentType::default())
                        .build(),
                    ParamSpecBoolean::builder("is-image").build(),
                    ParamSpecBoolean::builder("is-video").build(),
                    ParamSpecBoolean::builder("is-audio").build(),
                    ParamSpecBoolean::builder("is-file").build(),
                    ParamSpecString::builder("name").build(),
                    ParamSpecString::builder("content-type").build(),
                    ParamSpecBoolean::builder("loaded").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "image" => self.image.borrow().as_ref().to_value(),
                "video" => self.video.borrow().as_ref().to_value(),
                "audio" => self.audio.borrow().as_ref().to_value(),
                "file" => self.file.borrow().as_ref().to_value(),
                "name" => self.name.borrow().as_ref().to_value(),
                "type" => self
                    .content_type
                    .borrow()
                    .as_ref()
                    .map(AttachmentType::from_content_type)
                    .unwrap_or_default()
                    .to_value(),
                "is-image" => self.obj().is_image().to_value(),
                "is-video" => self.obj().is_video().to_value(),
                "is-audio" => self.obj().is_audio().to_value(),
                "is-file" => self.obj().is_file().to_value(),
                "content-type" => self.content_type.borrow().as_ref().to_value(),
                "loaded" => self.loaded.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
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
                "audio" => {
                    let obj = value
                        .get::<Option<MediaStream>>()
                        .expect("Property `audio` of `Attachment` has to be of type `MediaStream`");

                    self.audio.replace(obj);
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
