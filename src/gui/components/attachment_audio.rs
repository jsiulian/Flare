use crate::prelude::*;

use gtk::Widget;

use crate::config::BASE_ID;
use crate::gio::Settings;
use crate::gui::attachment::Attachment;

gtk::glib::wrapper! {
    /// Audio message UI.
    pub struct AttachmentAudio(ObjectSubclass<imp::AttachmentAudio>)
        @extends gtk::Widget, Attachment,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl AttachmentAudio {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        let obj = Object::builder::<Self>()
            .property("attachment", attachment)
            .build();
        let imp = obj.imp();
        // Remove the play_button when the attachment is not loaded
        let play_button: Option<Widget> = imp.controls.first_child().and_then(|w| w.first_child());

        if let Some(play_button) = play_button {
            if let Ok(play_button) = play_button.downcast::<gtk::Button>() {
                if !Settings::new(BASE_ID).boolean("autodownload-voice-messages") {
                    attachment.connect_notify_local(
                        Some("loaded"),
                        clone!(
                            #[weak]
                            play_button,
                            #[weak]
                            imp,
                            move |_, _| {
                                play_button.set_can_target(true);
                                play_button.set_visible(true);
                                imp.controls.set_width_request(250);
                                imp.controls.media_stream().unwrap().play();
                            }
                        ),
                    );
                    play_button.set_visible(false);
                }
            } else {
                log::warn!("'play_button' in 'AttachmentAudio' is not a 'Button'")
            }
        } else {
            log::warn!("Unable to find 'play_button' widget in 'AttachmentAudio'")
        }

        obj
    }
}

pub mod imp {
    use crate::prelude::*;

    use gtk::{Button, CompositeTemplate, Grid, MediaControls};

    use crate::gui::{attachment::Attachment, attachment::AttachmentImpl, utility::Utility};

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/components/attachment_audio.ui")]
    pub struct AttachmentAudio {
        #[template_child]
        box_audio: TemplateChild<Grid>,
        #[template_child]
        download_btn: TemplateChild<Button>,
        #[template_child]
        pub(super) controls: TemplateChild<MediaControls>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AttachmentAudio {
        const NAME: &'static str = "FlAttachmentAudio";
        type Type = super::AttachmentAudio;
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

    #[gtk::template_callbacks]
    impl AttachmentAudio {
        #[template_callback]
        pub async fn download(&self) {
            let obj = self.obj();
            let att: Attachment = obj.clone().upcast();
            att.imp().load();
            let btn = &obj.imp().download_btn;
            btn.set_can_target(false);
            let spinner = gtk::Spinner::new();
            spinner.start();
            btn.set_child(Some(&spinner));
        }
    }

    impl ObjectImpl for AttachmentAudio {
        fn constructed(&self) {
            self.parent_constructed();
        }

        fn dispose(&self) {
            self.box_audio.unparent()
        }
    }

    impl WidgetImpl for AttachmentAudio {}
    impl AttachmentImpl for AttachmentAudio {}
}
