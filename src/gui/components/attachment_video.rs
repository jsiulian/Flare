use crate::gui::attachment::Attachment;
use glib::Object;
use gtk::glib;

gtk::glib::wrapper! {
    pub struct AttachmentVideo(ObjectSubclass<imp::AttachmentVideo>)
        @extends gtk::Widget, Attachment;
}

impl AttachmentVideo {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::builder::<Self>()
            .property("attachment", attachment)
            .build()
    }
}

pub mod imp {

    use crate::gui::{attachment::Attachment, attachment::AttachmentImpl, utility::Utility};
    use glib::subclass::InitializingObject;
    use gtk::traits::WidgetExt;
    use gtk::{glib, Video};
    use gtk::{subclass::prelude::*, CompositeTemplate};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_video.ui")]
    pub struct AttachmentVideo {
        #[template_child]
        video: TemplateChild<Video>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentVideo {
        const NAME: &'static str = "FlAttachmentVideo";
        type Type = super::AttachmentVideo;
        type ParentType = Attachment;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
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
            self.video.unparent()
        }
    }

    impl WidgetImpl for AttachmentVideo {}
    impl AttachmentImpl for AttachmentVideo {}
}
