use crate::prelude::*;

use libsignal_service::profile_name::ProfileName;

#[macro_export]
macro_rules! gspawn {
    ($future:expr) => {
        let ctx = glib::MainContext::default();
        ctx.spawn_local($future);
    };
}

#[macro_export]
macro_rules! tspawn {
    ($future:expr) => {
        $crate::TOKIO_RUNTIME.spawn($future)
    };
}

#[cfg(target_os = "linux")]
pub fn is_flatpak() -> bool {
    let file = gio::File::for_path("/.flatpak-info");
    file.query_exists(gio::Cancellable::NONE)
}

pub fn format_profile_name(p: &ProfileName<String>) -> String {
    if let Some(family_name) = &p.family_name {
        format!("{} {}", p.given_name, family_name)
    } else {
        p.given_name.clone()
    }
}

pub fn chrono_to_glib_datetime(chrono: chrono::DateTime<chrono::Utc>) -> Option<glib::DateTime> {
    glib::DateTime::from_unix_utc(chrono.timestamp())
        .ok()
        .and_then(|d| d.to_local().ok())
}
