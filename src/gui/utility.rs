use gdk::glib::{DateTime, Object};

pub struct Utility {}

#[gtk::template_callbacks(functions)]
impl Utility {
    #[template_callback]
    fn not(b: bool) -> bool {
        !b
    }

    #[template_callback(function)]
    fn or(b1: bool, b2: bool) -> bool {
        b1 || b2
    }

    #[template_callback]
    fn is_some(opt: Option<Object>) -> bool {
        opt.is_some()
    }

    #[template_callback]
    fn is_none(opt: Option<Object>) -> bool {
        opt.is_none()
    }

    #[template_callback(function)]
    pub(super) fn format_timestamp(timestamp: u64) -> Option<String> {
        let datetime = DateTime::from_unix_utc((timestamp / 1000).try_into().unwrap_or_default())
            .ok()
            .and_then(|d| d.to_local().ok());
        // How to format time. Should probably be %H:%M (meaning print hours from 00-23, then a :,
        // then minutes from 00-59). For a full list of supported identifiers, see <https://docs.gtk.org/glib/method.DateTime.format.html>
        let format = gettextrs::gettext("%H:%M");
        datetime
            .and_then(|d| d.format(&format).ok())
            .map(|s| s.into())
    }
}
