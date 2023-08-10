use crate::gui::attachment::Attachment;
use glib::Object;
use gtk::glib;

gtk::glib::wrapper! {
    pub struct AttachmentAudio(ObjectSubclass<imp::AttachmentAudio>)
        @extends gtk::Widget, Attachment;
}

impl AttachmentAudio {
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
    use gtk::{glib, MediaControls};
    use gtk::{subclass::prelude::*, CompositeTemplate};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_audio.ui")]
    pub struct AttachmentAudio {
        #[template_child]
        controls: TemplateChild<MediaControls>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentAudio {
        const NAME: &'static str = "FlAttachmentAudio";
        type Type = super::AttachmentAudio;
        type ParentType = Attachment;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AttachmentAudio {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            self.controls.unparent()
        }
    }

    impl WidgetImpl for AttachmentAudio {}
    impl AttachmentImpl for AttachmentAudio {}
}
