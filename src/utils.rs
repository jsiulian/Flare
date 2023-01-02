use gio::prelude::FileExt;

#[macro_export]
macro_rules! gspawn {
    ($future:expr) => {
        let ctx = glib::MainContext::default();
        ctx.spawn_local($future);
    };
}

pub fn is_flatpak() -> bool {
    let file = gio::File::for_path("/.flatpak-info");
    file.query_exists(gio::Cancellable::NONE)
}
