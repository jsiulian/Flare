use gdk::prelude::{ApplicationExt, ApplicationExtManual};
use glib::IsA;
use gtk::traits::{GtkWindowExt, WidgetExt};

use std::path::Path;

mod config;
use self::config::{APP_ID, GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_BYTES};

mod backend;
mod error;
mod gui;
mod hash_log;
mod storage;
mod utils;

pub use error::{ApplicationError, ConfigurationError};

fn init_resources() {
    let gbytes = gtk::glib::Bytes::from_static(RESOURCES_BYTES);
    let resource = gtk::gio::Resource::from_data(&gbytes).unwrap();

    gtk::gio::resources_register(&resource);
}

fn init_icons<P: IsA<gdk::Display>>(display: &P) {
    let icon_theme = gtk::IconTheme::for_display(display);

    icon_theme.add_resource_path("/");
}

fn init_internationalization() -> Result<(), Box<dyn std::error::Error>> {
    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR)?;
    gettextrs::textdomain(GETTEXT_PACKAGE)?;
    Ok(())
}

fn main() {
    env_logger::init();
    init_internationalization().expect("Failed to initialize internationalization");

    if utils::is_flatpak() {
        if let Some(xdg_runtime_dir) = glib::getenv("XDG_RUNTIME_DIR") {
            let path = Path::new(&xdg_runtime_dir).join("app").join(APP_ID);
            let _ = glib::setenv("TMPDIR", path, true);
        }
    }

    gtk::init().expect("Failed to initialize gtk");
    libadwaita::init().expect("Failed to initializa libadwaita");
    let app = libadwaita::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &libadwaita::Application) {
    init_resources();
    let window = crate::gui::Window::new(app);
    init_icons(&window.display());
    window.present();
}
