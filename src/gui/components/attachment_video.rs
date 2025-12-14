use crate::prelude::*;
use gdk::Texture;

use crate::config::BASE_ID;
use crate::gio::Settings;
use crate::gui::attachment::Attachment;

glib::wrapper! {
    /// Attachment displaying videos
    pub struct AttachmentVideo(ObjectSubclass<imp::AttachmentVideo>)
        @extends gtk::Widget, Attachment,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl AttachmentVideo {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");

        let preview = attachment.property::<Option<Texture>>("image");
        let obj = Object::builder::<Self>()
            .property("attachment", attachment)
            .build();

        if preview.is_some() {
            obj.imp().video.set_paintable(preview.as_ref());
        }

        if !Settings::new(BASE_ID).boolean("autodownload-videos") {
            attachment.connect_notify_local(
                Some("loaded"),
                glib::clone!(
                    #[weak]
                    obj,
                    move |_, _| {
                        if let Some(media_stream) = obj.imp().controls.media_stream() {
                            media_stream.play();
                        }
                    }
                ),
            );
        }

        obj
    }
}

pub mod imp {
    use crate::gui::{attachment::Attachment, attachment::AttachmentImpl};
    use crate::prelude::*;

    use glib::subclass::InitializingObject;
    use gtk::{CompositeTemplate, MediaControls, Overlay, Picture};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_video.ui")]
    pub struct AttachmentVideo {
        #[template_child]
        pub video_overlay: TemplateChild<Overlay>,
        #[template_child]
        pub video: TemplateChild<Picture>,
        #[template_child]
        pub controls: TemplateChild<MediaControls>,
    }

    #[gtk::template_callbacks]
    impl AttachmentVideo {
        #[template_callback]
        fn toggle(&self) {
            if let Some(media_stream) = self.controls.media_stream() {
                if media_stream.is_playing() {
                    media_stream.pause();
                } else {
                    media_stream.play();
                }
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentVideo {
        const NAME: &'static str = "FlAttachmentVideo";
        type Type = super::AttachmentVideo;
        type ParentType = Attachment;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AttachmentVideo {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            self.video_overlay.unparent()
        }
    }

    impl WidgetImpl for AttachmentVideo {}
    impl AttachmentImpl for AttachmentVideo {}
}
