use glib::{Object, ObjectExt};
use gtk::glib;

gtk::glib::wrapper! {
    pub struct Attachment(ObjectSubclass<imp::Attachment>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::gio::ActionGroup, gtk::gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget;
}

impl Attachment {
    pub fn new(attachment: &crate::backend::Attachment) -> Self {
        log::trace!("Initializing `Attachment`");
        Object::builder::<Self>()
            .property("attachment", attachment)
            .build()
    }

    pub fn attachment(&self) -> crate::backend::Attachment {
        self.property("attachment")
    }
}

pub mod imp {
    use std::cell::RefCell;

    use ashpd::{desktop::open_uri::OpenFileRequest, WindowIdentifier};
    use gio::Settings;
    use glib::{
        clone, once_cell::sync::Lazy, subclass::InitializingObject, ParamSpec, ParamSpecObject,
        Value,
    };
    use gtk::{gio, glib, FileChooserNative};
    use gtk::{
        prelude::*, subclass::prelude::*, CompositeTemplate, FileChooserAction, ResponseType,
    };

    use crate::{
        backend::Manager,
        config::APP_ID,
        gspawn,
        gui::{error_dialog::ErrorDialog, utility::Utility},
    };

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/ui/attachment.ui")]
    pub struct Attachment {
        attachment: RefCell<Option<crate::backend::Attachment>>,

        manager: RefCell<Option<Manager>>,
    }

    #[gtk::template_callbacks]
    impl Attachment {
        #[template_callback]
        fn load(&self, _: gtk::Button) {
            let obj = self.obj();
            gspawn!(clone!(@strong obj => async move {
                let attachment = obj.attachment();
                attachment.load().await
            }));
        }

        fn window(&self) -> crate::gui::window::Window {
            self.obj()
                .root()
                .expect("`Attachment` to have a root")
                .dynamic_cast::<crate::gui::Window>()
                .expect("Root of `Attachment` to be a `Window`.")
        }

        #[template_callback]
        fn open(&self, _: gtk::Button) {
            log::trace!("User requested to open attachment");
            if let Some(file) = self
                .attachment
                .borrow()
                .as_ref()
                .and_then(|a| a.open_file())
            {
                let obj = self.obj();

                gspawn!(clone!(@weak obj => async move {
                    let identifier = WindowIdentifier::from_native(&obj.native().unwrap()).await;
                    if let Err(e) = OpenFileRequest::default()
                                        .ask(false)
                                        .identifier(identifier)
                                        .build_file(&file)
                                        .await {
                        log::error!("Failed to open file: {}", e);
                    }
                }));
            }
        }

        #[template_callback]
        fn download(&self, _: gtk::Button) {
            log::trace!("User requested to dowload attachment");
            if let Some(attachment) = self.attachment.borrow().as_ref() {
                let chooser = FileChooserNative::builder()
                    .transient_for(&self.window())
                    .action(FileChooserAction::Save)
                    .build();
                if let Some(name) = attachment.name() {
                    crate::trace!("Setting filename to {:?}", &name);
                    chooser.set_current_name(&name);
                }
                // TODO: Does not work inside Flatpak.
                // let _ = chooser.set_current_folder(
                //     glib::user_special_dir(glib::UserDirectory::Downloads)
                //         .map(|p| gio::File::for_path(&p))
                //         .as_ref(),
                // );

                let obj = self.obj();
                chooser.connect_response(
                    clone!(@weak chooser, @weak attachment, @weak obj => move |_, action| {
                        if action == ResponseType::Accept {
                            log::trace!("User downloads attachment");
                            let file = chooser.file();
                            if let Some(file) = file {
                                gspawn!(clone!(@strong attachment, @strong obj => async move {
                                    if let Err(e) = attachment.save_to_file(&file).await {
                                        let root = obj.imp().window();
                                        let dialog = ErrorDialog::new(e.into(), &root);
                                        dialog.show();
                                    }
                                }));
                            } else {
                                log::trace!("Got no file to save the attachment to");
                            }
                        } else {
                            log::trace!("User did not save a attachment");
                        }
                    }),
                );
                log::trace!("Showing download popup");
                chooser.show();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Attachment {
        const NAME: &'static str = "FlAttachment";
        type Type = super::Attachment;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Utility::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Attachment {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<Manager>("manager")
                        .construct_only()
                        .build(),
                    ParamSpecObject::builder::<crate::backend::Attachment>("attachment").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "manager" => self.manager.borrow().as_ref().to_value(),
                "attachment" => self.attachment.borrow().as_ref().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "manager" => {
                    let man = value
                        .get::<Option<Manager>>()
                        .expect("Property `manager` of `Attachment` has to be of type `Manager`");
                    self.manager.replace(man);
                }
                "attachment" => {
                    let att = value
                        .get::<Option<crate::backend::Attachment>>()
                        .expect("Property `attachment` of `Attachment` has to be of type `crate::backend::Attachment`");

                    let settings = Settings::new(APP_ID);
                    let autoload = att.is_some()
                        && settings.boolean("autodownload-images")
                        && att.as_ref().unwrap().is_image()
                        || settings.boolean("autodownload-videos")
                            && att.as_ref().unwrap().is_video()
                        || settings.boolean("autodownload-files")
                            && att.as_ref().unwrap().is_file();

                    self.attachment.replace(att);
                    if autoload {
                        log::trace!("Autodownloading attachment");
                        self.load(gtk::Button::new());
                    }
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Attachment {}
    impl BoxImpl for Attachment {}
}
