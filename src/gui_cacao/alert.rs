use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CString;
use std::path::PathBuf;

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

/// Show a modal NSOpenPanel and return the selected file path, if any.
pub fn pick_file() -> Option<PathBuf> {
    unsafe {
        let cls = class!(NSOpenPanel);
        let panel: *mut Object = msg_send![cls, openPanel];

        let _: () = msg_send![panel, setCanChooseFiles: objc::runtime::YES];
        let _: () = msg_send![panel, setCanChooseDirectories: objc::runtime::NO];
        let _: () = msg_send![panel, setAllowsMultipleSelection: objc::runtime::NO];

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
