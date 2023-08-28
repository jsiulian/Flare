use super::Manager;
use crate::gui::utility::Utility;
use blurhash;
use gdk::{Paintable, Texture};
use gio::{prelude::*, subclass::prelude::ObjectSubclassIsExt, Cancellable, File, FileCreateFlags};
use glib::{Bytes, Object, Priority};
use gtk::prelude::{PaintableExt, TextureExt};
use gtk::{gdk, gio, glib};
use gtk::{MediaFile, MediaStream};
use presage::prelude::{content::AttachmentPointer, AttachmentSpec};
use std::path::PathBuf;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, glib::Enum, Default)]
#[repr(u32)]
#[enum_type(name = "FlAttachmentType")]
pub enum AttachmentType {
    Image,
    Video,
    Audio,
    Gif,
    #[default]
    File,
}

#[derive(Debug, Clone, Copy, glib::Enum, Default)]
#[repr(i32)]
#[enum_type(name = "FlFlags")]
pub enum Flags {
    VoiceMessage,
    Borderless,
    Gif,
    #[default]
    None,
}

impl From<u32> for Flags {
    fn from(number: u32) -> Self {
        match number {
            0 => Self::None,
            1 => Self::VoiceMessage,
            2 => Self::Borderless,
            4 => Self::Gif,
            _ => unimplemented!(),
        }
    }
}

impl AttachmentType {
    fn from_content_type<S: AsRef<str>>(content: S) -> Self {
        let content = content.as_ref();
        if content.starts_with("image/gif") {
            Self::Gif
        } else if content.starts_with("image/") {
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
        let mut size = 0;
        if let Ok(file_info) = file.query_info(
            gio::FILE_ATTRIBUTE_STANDARD_SIZE,
            gio::FileQueryInfoFlags::NONE,
            Cancellable::NONE,
        ) {
            size = file_info.size() as u32;
        };

        let obj = Object::builder::<Self>()
            .property("manager", manager)
            .property("file", &file)
            .property(
                "name",
                &file
                    .basename()
                    .and_then(|f| f.file_name().map(|s| s.to_string_lossy().into_owned())),
            )
            .property("size", size)
            .property("loaded", true)
            .property("content-type", &mime)
            .build();

        if mime.starts_with("image/") {
            let image = Texture::from_file(&file).ok().and_upcast::<Paintable>();
            obj.set_property("image", &image);

            if let Some(paintable) = image.as_ref() {
                obj.set_property("width", paintable.intrinsic_width() as u32);
                obj.set_property("height", paintable.intrinsic_height() as u32);
            }
        }

        if mime.starts_with("video/") {
            let media_file = MediaFile::for_file(&file);
            obj.set_property("video", &media_file);

            media_file.connect_closure(
                "invalidate-size",
                false,
                glib::closure_local!(@watch obj => move |media_file: MediaFile| {
                        obj.set_property("image", media_file.current_image());
                        obj.set_property("width", media_file.intrinsic_width() as u32);
                        obj.set_property("height", media_file.intrinsic_height() as u32);
                }),
            );
        }

        if mime.starts_with("audio/") {
            obj.set_property("audio", Some(MediaFile::for_file(&file)));
        }
        obj
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
            .property("width", texture.width() as u32)
            .property("height", texture.height() as u32)
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

    pub fn is_gif(&self) -> bool {
        self.attachment_type() == AttachmentType::Gif
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
        let img_width: Option<u32> = image.as_ref().and_then(|i| i.width().try_into().ok());
        let img_height: Option<u32> = image.as_ref().and_then(|i| i.height().try_into().ok());

        let img_blur_hash = match image {
            Some(image) => {
                let bytes = image.save_to_png_bytes().to_vec();
                let img =
                    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).unwrap();
                // components_x - The number of components in the X direction. Must be between 1 and 9. 3 to 5 is usually a good range for this.
                // components_y - The number of components in the Y direction. Must be between 1 and 9. 3 to 5 is usually a good range for this.
                blurhash::encode(
                    5,
                    5,
                    img_width.unwrap(),
                    img_height.unwrap(),
                    &img.to_rgba8().into_vec(),
                )
                .ok()
            }
            None => None,
        };

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
                width: img_width,
                height: img_height,
                caption: None,
                blur_hash: img_blur_hash,
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
        let mut blur_hash = None;
        let mut name = None;
        let mut size = 0;
        let mut width = 0;
        let mut height = 0;

        // Populate blur_hash if set.
        if let Some(pointer_blurhash) = &pointer.blur_hash {
            blur_hash = Some(pointer_blurhash.clone());
        }
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

        // Populate size if set.
        if let Some(pointer_size) = &pointer.size {
            size = *pointer_size;
        }

        // Populate width if set.
        if let Some(pointer_width) = &pointer.width {
            width = *pointer_width;
        }
        // Populate height if set.
        if let Some(pointer_height) = &pointer.height {
            height = *pointer_height;
        }

        if let Some(blur_hash) = &pointer.blur_hash {
            let actual_width = Utility::resize_width(width);
            let actual_height = Utility::resize_height(width, height);

            if let Ok(buffer) = blurhash::decode_pixbuf(
                blur_hash.as_str(),
                actual_width as u32,
                actual_height as u32,
                1.0,
            ) {
                image = Some(Texture::for_pixbuf(&buffer));
            }
        }

        let s: Self = Object::builder::<Self>()
            .property("manager", manager)
            .property("image", image)
            .property("name", name)
            .property("size", size)
            .property("video", &None::<MediaStream>)
            .property("audio", &None::<MediaStream>)
            .property("loaded", false)
            .property("blur-hash", blur_hash)
            .property("width", width)
            .property("height", height)
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
        let name = self.property::<String>("name");
        let mut file = None;

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

        self.notify("type");
        self.notify("flags");
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

    pub fn size(&self) -> u32 {
        self.property::<u32>("size")
    }

    pub fn loaded(&self) -> bool {
        self.property::<bool>("loaded")
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
    use gdk::{subclass::prelude::*, Paintable};
    use gio::File;
    use glib::ParamSpecEnum;
    use glib::{
        once_cell::sync::Lazy, Bytes, ParamSpec, ParamSpecBoolean, ParamSpecObject,
        ParamSpecString, ParamSpecUInt, Value,
    };
    use gtk::MediaStream;
    use gtk::{gdk, gio, glib};
    use presage::prelude::content::AttachmentPointer;

    use crate::backend::attachment::{AttachmentType, Flags};
    use crate::backend::Manager;

    #[derive(Default)]
    pub struct Attachment {
        pub(crate) image: RefCell<Option<Paintable>>,
        video: RefCell<Option<MediaStream>>,
        audio: RefCell<Option<MediaStream>>,
        file: RefCell<Option<File>>,
        name: RefCell<Option<String>>,
        size: Cell<u32>,
        width: Cell<u32>,
        height: Cell<u32>,

        blur_hash: RefCell<Option<String>>,
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
                    ParamSpecObject::builder::<Paintable>("image").build(),
                    ParamSpecObject::builder::<MediaStream>("video").build(),
                    ParamSpecObject::builder::<MediaStream>("audio").build(),
                    ParamSpecObject::builder::<File>("file").build(),
                    ParamSpecEnum::builder::<AttachmentType>("type")
                        .read_only()
                        .default_value(AttachmentType::default())
                        .build(),
                    ParamSpecEnum::builder::<Flags>("flags").read_only().build(),
                    ParamSpecBoolean::builder("is-image").build(),
                    ParamSpecBoolean::builder("is-video").build(),
                    ParamSpecBoolean::builder("is-audio").build(),
                    ParamSpecBoolean::builder("is-file").build(),
                    ParamSpecString::builder("name").build(),
                    ParamSpecUInt::builder("size").build(),
                    ParamSpecUInt::builder("width").build(),
                    ParamSpecUInt::builder("height").build(),
                    ParamSpecString::builder("content-type").build(),
                    ParamSpecString::builder("blur-hash").build(),
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
                "size" => self.size.get().to_value(),
                "width" => self.width.get().to_value(),
                "height" => self.height.get().to_value(),
                "type" => self
                    .content_type
                    .borrow()
                    .as_ref()
                    .map(AttachmentType::from_content_type)
                    .unwrap_or_default()
                    .to_value(),
                "flags" => Flags::from(self.pointer.borrow().as_ref().unwrap().flags()).to_value(),
                "is-image" => self.obj().is_image().to_value(),
                "is-gif" => self.obj().is_gif().to_value(),
                "is-video" => self.obj().is_video().to_value(),
                "is-audio" => self.obj().is_audio().to_value(),
                "is-file" => self.obj().is_file().to_value(),
                "content-type" => self.content_type.borrow().as_ref().to_value(),
                "blur-hash" => self.blur_hash.borrow().as_ref().to_value(),
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
                        .get::<Option<Paintable>>()
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
                "size" => {
                    let obj = value
                        .get::<u32>()
                        .expect("Property `size` of `Attachment` has to be of type `u32`");

                    self.size.replace(obj);
                }
                "width" => {
                    let obj = value
                        .get::<u32>()
                        .expect("Property `width` of `Attachment` has to be of type `u32`");

                    self.width.replace(obj);
                }
                "height" => {
                    let obj = value
                        .get::<u32>()
                        .expect("Property `height` of `Attachment` has to be of type `u32`");

                    self.height.replace(obj);
                }
                "content-type" => {
                    let obj = value.get::<Option<String>>().expect(
                        "Property `content-type` of `Attachment` has to be of type `String`",
                    );

                    self.content_type.replace(obj);
                }
                "blur-hash" => {
                    let obj = value
                        .get::<Option<String>>()
                        .expect("Property `blur-hash` of `Attachment` has to be of type `String`");

                    self.blur_hash.replace(obj);
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
