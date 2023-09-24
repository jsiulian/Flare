use gdk::gio;

use gtk::glib::{DateTime, Object};

pub struct Utility {}

const MAX_MEDIA_WIDTH: i32 = 250;

#[gtk::template_callbacks(functions)]
impl Utility {
    #[template_callback]
    fn network_monitor() -> gio::NetworkMonitor {
        gio::NetworkMonitor::default()
    }

    #[template_callback]
    fn not(b: bool) -> bool {
        !b
    }

    #[template_callback(function)]
    fn or(b1: bool, b2: bool) -> bool {
        b1 || b2
    }

    #[template_callback(function)]
    fn and(b1: bool, b2: bool) -> bool {
        b1 && b2
    }

    #[template_callback]
    fn is_some(opt: Option<Object>) -> bool {
        opt.is_some()
    }

    #[template_callback]
    fn is_none(opt: Option<Object>) -> bool {
        opt.is_none()
    }

    #[template_callback]
    fn is_empty(s: Option<String>) -> bool {
        s.map(|s| s.is_empty()).unwrap_or_default()
    }

    #[template_callback]
    fn not_empty(s: Option<String>) -> bool {
        s.map(|s| !s.is_empty()).unwrap_or_default()
    }

    #[template_callback]
    fn uint_equal(i1: u32, i2: i32) -> bool {
        i1 as i32 == i2
    }

    #[template_callback]
    pub(crate) fn resize_width(width: u32) -> i32 {
        if (width as i32) < MAX_MEDIA_WIDTH {
            width as i32
        } else {
            MAX_MEDIA_WIDTH
        }
    }

    #[template_callback]
    pub(crate) fn resize_height(width: u32, height: u32) -> i32 {
        if height > 0 && width > 0 {
            let aspect_ratio: f32 = width as f32 / height as f32;
            if (width as i32) < MAX_MEDIA_WIDTH {
                height as i32
            } else {
                (MAX_MEDIA_WIDTH as f32 / aspect_ratio) as i32
            }
        } else {
            -1
        }
    }

    #[template_callback(function)]
    pub(super) fn format_timestamp(timestamp: u64) -> Option<String> {
        let datetime = DateTime::from_unix_utc((timestamp / 1000).try_into().unwrap_or_default())
            .ok()
            .and_then(|d| d.to_local().ok());
        let now = DateTime::now_local().expect("Now to be representable as DateTime");
        let difference = datetime.as_ref().map(|d| now.difference(d).as_hours());
        // How to format time. Should probably be %H:%M (meaning print hours from 00-23, then a :,
        // then minutes from 00-59). For a full list of supported identifiers, see <https://docs.gtk.org/glib/method.DateTime.format.html>
        let format = gettextrs::gettext("%H:%M");
        // How to format a date with time. Should probably be similar to %Y-%m-%d %H:%M (meaning print year, month from 01-12, day from 01-31 (each separated by -), hours from 00-23, then a :,
        // then minutes from 00-59). For a full list of supported identifiers, see <https://docs.gtk.org/glib/method.DateTime.format.html>
        let format_full = gettextrs::gettext("%Y-%m-%d %H:%M");
        datetime
            .and_then(|d| {
                if difference.is_some() && difference.unwrap() >= 24 {
                    d.format(&format_full).ok()
                } else {
                    d.format(&format).ok()
                }
            })
            .map(|s| s.into())
    }

    #[template_callback(function)]
    pub(super) fn format_size(size: u32) -> gtk::glib::GString {
        gtk::glib::format_size(size as u64)
    }

    #[template_callback(function)]
    pub(super) fn format_time_human(time: &DateTime) -> Option<gtk::glib::GString> {
        // How to format time. Should probably be %H:%M (meaning print hours from 00-23, then a :,
        // then minutes from 00-59). For a full list of supported identifiers, see <https://docs.gtk.org/glib/method.DateTime.format.html>
        let format = gettextrs::gettext("%H:%M");
        time.format(&format).ok()
    }

    // Should not really fail.
    pub(super) fn format_date_human(date: &DateTime) -> Option<gtk::glib::GString> {
        let now = DateTime::now_local().expect("Now to be representable as DateTime");

        let today = now.day_of_year() == date.day_of_year() && now.year() == date.year();
        let yesterday = now.day_of_year() == date.day_of_year() + 1 && now.year() == date.year();

        if today {
            return Some(gettextrs::gettext("Today").into());
        } else if yesterday {
            return Some(gettextrs::gettext("Yesterday").into());
        }

        let format = if now.year() != date.year() {
            // How to format a human-readable date including the year. Should probably be similar to %Y-%m-%d (meaning print year, month from 01-12, day from 01-31 (each separated by -)).
            // For a full list of supported identifiers, see <https://docs.gtk.org/glib/method.DateTime.format.html>
            gettextrs::gettext("%Y-%m-%d")
        } else {
            // How to format a human-readable date excluding the year. Should probably be similar to "%a., %d. %b" (meaning print abbreviated weekday, day from 1-31 and abbreviated month name).
            // For a full list of supported identifiers, see <https://docs.gtk.org/glib/method.DateTime.format.html>
            gettextrs::gettext("%a., %e. %b")
        };

        date.format(&format).ok()
    }
}
