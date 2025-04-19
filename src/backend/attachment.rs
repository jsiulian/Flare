use super::Manager;
use crate::prelude::*;

use gdk::{Paintable, Texture};
use gio::{Cancellable, File, FileCreateFlags};
use glib::{Bytes, Object, Priority};
use gtk::{MediaFile, MediaStream};

use libsignal_service::proto::AttachmentPointer;
use libsignal_service::sender::AttachmentSpec;

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
    /// Any attachment to a message, with some specific [AttachmentType].
    pub struct Attachment(ObjectSubclass<imp::Attachment>);
}

impl Attachment {
    pub fn from_file(file: File, manager: &Manager) -> Self {
        log::trace!("Trying to build a Attachment from a file");
        // For some reason, `gio::content_type_guess` returns `application/x-zerosize` for PNGs.
        // Hardcode to return the correct content type.
        let mime = if file
            .basename()
            .as_ref()
            .and_then(|b| b.extension())
            .is_some_and(|b| b == "png")
        {
            "image/png".to_string()
        } else {
            gio::content_type_guess(file.basename(), &[])
                .0
                .as_str()
                .to_owned()
        };
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
                file.basename()
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
                glib::closure_local!(
                    #[watch]
                    obj,
                    move |media_file: MediaFile| {
                        obj.set_property("image", media_file.current_image());
                        obj.set_property("width", media_file.intrinsic_width() as u32);
                        obj.set_property("height", media_file.intrinsic_height() as u32);
                    }
                ),
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

    /// Convert the attachment to an attachment that can be uploaded.
    ///
    /// This is currently only possible for pictures and files.
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

        // The blurhash is a short code for an image which looks like the image itself but heavily blurred.
        // This will be shown before the message is loaded.
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

        // For some reason, some webp stickers are sent with content type Some("").
        // Also, some stickers mention the completely wrong file type (image/webp instead of image/png), but that should not matter as both are equally handled by gtk::Picture.
        let mut pointer = pointer.clone();
        if pointer.content_type == Some("".to_owned()) {
            pointer.content_type = Some("image/webp".to_owned());
        }

        let mut image = None;
        let blur_hash = pointer.blur_hash.clone();
        let name = pointer.file_name.clone().unwrap_or_else(|| {
            match &pointer.content_type {
                Some(t) if t.starts_with("image/") => format!("image.{}", &t[6..]),
                Some(t) if t.starts_with("video/") => format!("video.{}", &t[6..]),
                Some(t) if t.starts_with("audio/") => format!("audio.{}", &t[6..]),
                // This should never happen.
                _ => "file".to_string(),
            }
        });
        let size = pointer.size.unwrap_or_default();
        let width = pointer.width.unwrap_or_default();
        let height = pointer.height.unwrap_or_default();

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
        *s.imp().pointer.borrow_mut() = Some(pointer);
        s
    }

    /// Download the attachment for display.
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
    use std::marker::PhantomData;

    use super::{AttachmentType, Flags};
    use crate::backend::Manager;
    use crate::prelude::*;

    use gdk::Paintable;
    use gio::File;
    use glib::Bytes;
    use gtk::MediaStream;

    use libsignal_service::proto::AttachmentPointer;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Attachment)]
    pub struct Attachment {
        #[property(get, set)]
        pub(crate) image: RefCell<Option<Paintable>>,
        #[property(get, set)]
        video: RefCell<Option<MediaStream>>,
        #[property(get, set)]
        audio: RefCell<Option<MediaStream>>,
        #[property(get, set)]
        file: RefCell<Option<File>>,
        #[property(get, set)]
        name: RefCell<Option<String>>,
        #[property(get, set)]
        size: Cell<u32>,
        #[property(get, set)]
        width: Cell<u32>,
        #[property(get, set)]
        height: Cell<u32>,

        #[property(get, set)]
        blur_hash: RefCell<Option<String>>,
        #[property(get, set)]
        content_type: RefCell<Option<String>>,

        #[property(get, set)]
        loaded: Cell<bool>,

        #[property(get = Self::r#type, builder(AttachmentType::default()))]
        r#type: PhantomData<AttachmentType>,
        #[property(get = |s: &Self| s.r#type() == AttachmentType::Image)]
        is_image: PhantomData<bool>,
        #[property(get = |s: &Self| s.r#type() == AttachmentType::Video)]
        is_video: PhantomData<bool>,
        #[property(get = |s: &Self| s.r#type() == AttachmentType::Gif)]
        is_gif: PhantomData<bool>,
        #[property(get = |s: &Self| s.r#type() == AttachmentType::Audio)]
        is_audio: PhantomData<bool>,
        #[property(get = |s: &Self| s.r#type() == AttachmentType::File)]
        is_file: PhantomData<bool>,

        #[property(get = Self::flags, builder(Flags::default()))]
        flags: PhantomData<Flags>,

        pub(super) pointer: RefCell<Option<AttachmentPointer>>,
        pub(super) raw: RefCell<Option<Bytes>>,

        #[property(get, set, construct_only, type=Manager)]
        manager: RefCell<Option<Manager>>,
    }

    impl Attachment {
        fn r#type(&self) -> AttachmentType {
            self.content_type
                .borrow()
                .as_ref()
                .map(AttachmentType::from_content_type)
                .unwrap_or_default()
        }

        fn flags(&self) -> Flags {
            Flags::from(self.pointer.borrow().as_ref().unwrap().flags())
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Attachment {
        const NAME: &'static str = "FlAttachmentObject";
        type Type = super::Attachment;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Attachment {}
}
