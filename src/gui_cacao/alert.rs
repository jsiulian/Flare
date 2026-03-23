use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CString;
use std::path::PathBuf;

use cacao::geometry::Rect;

unsafe fn nsstring(s: &str) -> *mut Object {
    let cs = CString::new(s).unwrap_or_default();
    let cls = class!(NSString);
    let obj: *mut Object = msg_send![cls, alloc];
    msg_send![obj, initWithUTF8String: cs.as_ptr()]
}

/// Show a modal NSAlert with a confirm button and a cancel button.
/// Returns true if the user clicked the confirm (first) button.
pub fn confirm(title: &str, message: &str, confirm_label: &str, cancel_label: &str) -> bool {
    unsafe {
        let cls = class!(NSAlert);
        let alert: *mut Object = msg_send![cls, new];

        let ns_title = nsstring(title);
        let _: () = msg_send![alert, setMessageText: ns_title];

        let ns_msg = nsstring(message);
        let _: () = msg_send![alert, setInformativeText: ns_msg];

        let ns_confirm = nsstring(confirm_label);
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_confirm];

        let ns_cancel = nsstring(cancel_label);
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_cancel];

        // Center the alert on screen
        if let Some(key_window) = get_key_window() {
            let _: () = msg_send![key_window, center];
        }

        // NSAlertFirstButtonReturn = 1000
        let response: i64 = msg_send![alert, runModal];
        response == 1000
    }
}

/// Show a modal NSAlert with an informational message (OK only).
pub fn info(title: &str, message: &str) {
    unsafe {
        let cls = class!(NSAlert);
        let alert: *mut Object = msg_send![cls, new];

        let ns_title = nsstring(title);
        let _: () = msg_send![alert, setMessageText: ns_title];

        let ns_msg = nsstring(message);
        let _: () = msg_send![alert, setInformativeText: ns_msg];

        // Center the alert on screen
        if let Some(key_window) = get_key_window() {
            let _: () = msg_send![key_window, center];
        }

        let _: i64 = msg_send![alert, runModal];
    }
}

/// Show a modal NSAlert with an error message and optional Report button.
/// Returns "report" if Report button clicked, "ok" if OK button clicked, or None on cancel.
pub fn error_with_report(title: &str, message: &str, show_report: bool) -> Option<String> {
    unsafe {
        let cls = class!(NSAlert);
        let alert: *mut Object = msg_send![cls, new];

        let ns_title = nsstring(title);
        let _: () = msg_send![alert, setMessageText: ns_title];

        let ns_msg = nsstring(message);
        let _: () = msg_send![alert, setInformativeText: ns_msg];

        // NSAlertSecondButtonReturn = 1001
        if show_report {
            let ns_report = nsstring("Report");
            let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_report];
        }

        let ns_ok = nsstring("OK");
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_ok];

        // Center the alert on screen
        if let Some(key_window) = get_key_window() {
            let _: () = msg_send![key_window, center];
        }

        let response: i64 = msg_send![alert, runModal];
        if show_report && response == 1001 {
            Some("report".to_string())
        } else {
            Some("ok".to_string())
        }
    }
}

/// Open a URL in the default browser (for Report button).
pub fn open_url(url: &str) {
    unsafe {
        let ns_url = nsstring(url);
        let nsworkspace = class!(NSWorkspace);
        let shared: *mut Object = msg_send![nsworkspace, sharedWorkspace];
        let _: () = msg_send![shared, openURL: ns_url];
    }
}

unsafe fn get_key_window() -> Option<*mut Object> {
    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
    let key_window: *mut Object = msg_send![app, keyWindow];
    if key_window.is_null() {
        None
    } else {
        Some(key_window)
    }
}

/// Read a bool value from NSUserDefaults.
pub fn user_default_bool(key: &str) -> Option<bool> {
    unsafe {
        let cls = class!(NSUserDefaults);
        let defaults: *mut Object = msg_send![cls, standardUserDefaults];
        let ns_key = nsstring(key);
        // Check if key exists
        let obj: *mut Object = msg_send![defaults, objectForKey: ns_key];
        if obj.is_null() {
            return None;
        }
        let val: bool = msg_send![defaults, boolForKey: ns_key];
        Some(val)
    }
}

/// Write a bool value to NSUserDefaults.
pub fn set_user_default_bool(key: &str, value: bool) {
    unsafe {
        let cls = class!(NSUserDefaults);
        let defaults: *mut Object = msg_send![cls, standardUserDefaults];
        let ns_key = nsstring(key);
        let _: () = msg_send![defaults, setBool: value forKey: ns_key];
        let _: () = msg_send![defaults, synchronize];
    }
}

/// Read a color (RGBA hex string) from NSUserDefaults.
/// Returns (r, g, b, a) components as f64 in [0, 1].
pub fn user_default_color(key: &str) -> Option<(f64, f64, f64, f64)> {
    unsafe {
        let cls = class!(NSUserDefaults);
        let defaults: *mut Object = msg_send![cls, standardUserDefaults];
        let ns_key = nsstring(key);
        let obj: *mut Object = msg_send![defaults, objectForKey: ns_key];
        if obj.is_null() {
            return None;
        }
        let ns_str: *mut Object = msg_send![obj, description];
        let len: usize = msg_send![ns_str, lengthOfBytesUsingEncoding: 4i64]; // NSASCIIStringEncoding
        let mut buffer = vec![0u8; len];
        let _: usize =
            msg_send![ns_str, getCString: buffer.as_mut_ptr() maxLength: len encoding: 4i64];

        // Parse hex string like "#RRGGBBAA" or "RRGGBBAA"
        let hex_str = std::str::from_utf8(&buffer).ok()?;
        let hex_str = hex_str.trim_start_matches('#');
        if hex_str.len() < 8 {
            return None;
        }
        let r = u8::from_str_radix(&hex_str[0..2], 16).ok()? as f64 / 255.0;
        let g = u8::from_str_radix(&hex_str[2..4], 16).ok()? as f64 / 255.0;
        let b = u8::from_str_radix(&hex_str[4..6], 16).ok()? as f64 / 255.0;
        let a = u8::from_str_radix(&hex_str[6..8], 16).ok()? as f64 / 255.0;
        Some((r, g, b, a))
    }
}

/// Write a color (RGBA hex string) to NSUserDefaults.
pub fn set_user_default_color(key: &str, r: f64, g: f64, b: f64, a: f64) {
    let r = (r.clamp(0.0, 1.0) * 255.0) as u8;
    let g = (g.clamp(0.0, 1.0) * 255.0) as u8;
    let b = (b.clamp(0.0, 1.0) * 255.0) as u8;
    let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
    let hex_str = format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a);

    unsafe {
        let cls = class!(NSUserDefaults);
        let defaults: *mut Object = msg_send![cls, standardUserDefaults];
        let ns_key = nsstring(key);
        let ns_val = nsstring(&hex_str);
        let _: () = msg_send![defaults, setObject: ns_val forKey: ns_key];
        let _: () = msg_send![defaults, synchronize];
    }
}

/// Show a modal NSOpenPanel and return the selected file path, if any.
pub fn pick_file() -> Option<PathBuf> {
    unsafe {
        let cls = class!(NSOpenPanel);
        let panel: *mut Object = msg_send![cls, openPanel];

        let _: () = msg_send![panel, setCanChooseFiles: objc::runtime::YES];
        let _: () = msg_send![panel, setCanChooseDirectories: objc::runtime::NO];
        let _: () = msg_send![panel, setAllowsMultipleSelection: objc::runtime::NO];

        // Center the panel on screen
        if let Some(key_window) = get_key_window() {
            let _: () = msg_send![key_window, center];
        }

        // NSModalResponseOK = 1
        let response: i64 = msg_send![panel, runModal];
        if response != 1 {
            return None;
        }

        let url: *mut Object = msg_send![panel, URL];
        if url.is_null() {
            return None;
        }

        let path: *mut Object = msg_send![url, path];
        if path.is_null() {
            return None;
        }

        let utf8: *const i8 = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }

        let cstr = std::ffi::CStr::from_ptr(utf8);
        cstr.to_str().ok().map(PathBuf::from)
    }
}

/// Show a modal NSOpenPanel and return the selected file paths.
pub fn pick_files() -> Vec<PathBuf> {
    unsafe {
        let cls = class!(NSOpenPanel);
        let panel: *mut Object = msg_send![cls, openPanel];

        let _: () = msg_send![panel, setCanChooseFiles: objc::runtime::YES];
        let _: () = msg_send![panel, setCanChooseDirectories: objc::runtime::NO];
        let _: () = msg_send![panel, setAllowsMultipleSelection: objc::runtime::YES];

        // Center the panel on screen
        if let Some(key_window) = get_key_window() {
            let _: () = msg_send![key_window, center];
        }

        // NSModalResponseOK = 1
        let response: i64 = msg_send![panel, runModal];
        if response != 1 {
            return Vec::new();
        }

        let urls: *mut Object = msg_send![panel, URLs];
        if urls.is_null() {
            return Vec::new();
        }

        let count: usize = msg_send![urls, count];
        let mut paths = Vec::new();

        for i in 0..count {
            let url: *mut Object = msg_send![urls, objectAtIndex: i];
            if url.is_null() {
                continue;
            }
            let path: *mut Object = msg_send![url, path];
            if path.is_null() {
                continue;
            }
            let utf8: *const i8 = msg_send![path, UTF8String];
            if utf8.is_null() {
                continue;
            }
            let cstr = std::ffi::CStr::from_ptr(utf8);
            if let Some(s) = cstr.to_str().ok() {
                paths.push(PathBuf::from(s));
            }
        }

        paths
    }
}

/// Show a modal dialog for submitting captcha.
/// Returns Some((token, captcha)) if submitted, None if cancelled.
pub fn submit_captcha() -> Option<(String, String)> {
    unsafe {
        let cls = class!(NSAlert);
        let alert: *mut Object = msg_send![cls, new];

        let ns_title = nsstring("Submit Captcha");
        let _: () = msg_send![alert, setMessageText: ns_title];

        let body = "Submit a Captcha challenge. The token can be obtained from the error message that was displayed from Flare. The captcha must be filled out on signalcaptchas.org and the link to open Signal must be pasted to the corresponding entry. Note that the captcha is only valid for about one minute.";
        let ns_body = nsstring(body);
        let _: () = msg_send![alert, setInformativeText: ns_body];

        let ns_submit = nsstring("Submit");
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_submit];

        let ns_cancel = nsstring("Cancel");
        let _: *mut Object = msg_send![alert, addButtonWithTitle: ns_cancel];

        // Create accessory view with text fields
        let accessory = create_accessory_view();
        let _: () = msg_send![alert, setAccessoryView: accessory];

        // Center the alert on screen
        if let Some(key_window) = get_key_window() {
            let _: () = msg_send![key_window, center];
        }

        let response: i64 = msg_send![alert, runModal];

        if response == 1000 {
            // Submit button (first button)
            let token = get_text_field_value(accessory, 1);
            let captcha = get_text_field_value(accessory, 2);
            if !token.is_empty() && !captcha.is_empty() {
                Some((token, captcha))
            } else {
                None
            }
        } else {
            None
        }
    }
}

unsafe fn create_accessory_view() -> *mut Object {
    let container: *mut Object = msg_send![class!(NSView), new];

    // Token field (created with new, no autorelease needed)
    let token_label = create_label("Token:");
    let token_field = create_text_field();

    // Captcha field
    let captcha_label = create_label("Captcha:");
    let captcha_field = create_text_field();

    // Add subviews (container retains them)
    let _: () = msg_send![container, addSubview: token_label];
    let _: () = msg_send![container, addSubview: token_field];
    let _: () = msg_send![container, addSubview: captcha_label];
    let _: () = msg_send![container, addSubview: captcha_field];

    // Layout: stacked vertically
    let width: f64 = 300.0;
    let label_height: f64 = 20.0;
    let field_height: f64 = 24.0;
    let padding: f64 = 20.0;

    // Token label
    let token_label_frame = Rect::new(padding, 80.0, 80.0, label_height);
    let _: () = msg_send![token_label, setFrame: token_label_frame];

    // Token field
    let token_field_frame = Rect::new(padding + 85.0, 76.0, width - 85.0 - padding, field_height);
    let _: () = msg_send![token_field, setFrame: token_field_frame];

    // Captcha label
    let captcha_label_frame = Rect::new(padding, 45.0, 80.0, label_height);
    let _: () = msg_send![captcha_label, setFrame: captcha_label_frame];

    // Captcha field
    let captcha_field_frame = Rect::new(padding + 85.0, 41.0, width - 85.0 - padding, field_height);
    let _: () = msg_send![captcha_field, setFrame: captcha_field_frame];

    // Set container size
    let container_frame = Rect::new(0.0, 0.0, width, 110.0);
    let _: () = msg_send![container, setFrame: container_frame];

    // Tag fields for retrieval (1 and 2)
    let _: () = msg_send![token_field, setTag: 1];
    let _: () = msg_send![captcha_field, setTag: 2];

    container
}

unsafe fn create_label(text: &str) -> *mut Object {
    let ns_text = nsstring(text);
    let label: *mut Object = msg_send![class!(NSTextField), labelWithString: ns_text];
    let font: *mut Object = msg_send![class!(NSFont), systemFontOfSize: 13.0];
    let _: () = msg_send![label, setFont: font];
    label
}

unsafe fn create_text_field() -> *mut Object {
    let field: *mut Object = msg_send![class!(NSTextField), new];
    let _: () = msg_send![field, setBezeled: objc::runtime::YES];
    let _: () = msg_send![field, setBezelStyle: 1i32]; // NSTextFieldRoundedBezel
    let _: () = msg_send![field, setEditable: objc::runtime::YES];
    let _: () = msg_send![field, setSelectable: objc::runtime::YES];
    let _: () = msg_send![field, setDrawsBackground: objc::runtime::YES];
    field
}

unsafe fn get_text_field_value(container: *mut Object, tag: u32) -> String {
    let view: *mut Object = msg_send![container, viewWithTag: tag as i32];
    if view.is_null() {
        return String::new();
    }
    let value: *mut Object = msg_send![view, stringValue];
    if value.is_null() {
        return String::new();
    }
    let c_str: *const std::os::raw::c_char = msg_send![value, UTF8String];
    let bytes = std::slice::from_raw_parts(c_str as *const u8, 4096);
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}
