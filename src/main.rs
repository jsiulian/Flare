use gdk::prelude::{ApplicationExt, ApplicationExtManual};
use gio::prelude::SettingsExt;
use gio::{ApplicationFlags, Settings, SettingsBindFlags};
use glib::IsA;
use gtk::prelude::SettingsExtManual;
use gtk::traits::{GtkWindowExt, WidgetExt};
use gtk::{gdk, gio, glib};
use once_cell::sync::Lazy;

use std::path::Path;

mod config;
use self::config::{APP_ID, GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_BYTES};

mod backend;
mod dbus;
mod error;
mod gui;
mod hash_log;
mod utils;

pub use error::{ApplicationError, ConfigurationError};

pub static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Runtime::new().unwrap());

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

    init_resources();

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    // Do not start as a service if setting not set
    // Background portal may have created a .desktop file in ~/.config/autostart
    if app.flags() & ApplicationFlags::IS_SERVICE == ApplicationFlags::IS_SERVICE {
        let settings = Settings::new(APP_ID);
        let run_in_background = settings.boolean("run-in-background");
        if !run_in_background {
            return;
        }
    }

    match app.register(gio::Cancellable::NONE) {
        Ok(_) => {}
        Err(err) => log::warn!("Registration error, {}", err),
    }

    if !app.is_remote() {
        build_ui(&app);
    }
    sourceview5::init();
    app.run();
}

fn build_ui(app: &adw::Application) {
    let settings = Settings::new(APP_ID);
    let window = crate::gui::Window::new(app);
    settings
        .bind("run-in-background", &window, "hide-on-close")
        .flags(SettingsBindFlags::DEFAULT)
        .build();
    init_icons(&window.display());
    app.connect_activate(move |_| {
        window.present();
    });
}
