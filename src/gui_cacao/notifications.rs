/// macOS UNUserNotificationCenter integration.
/// Posts a notification when a message arrives in a non-active channel,
/// and withdraws notifications when that channel is opened.
use std::ffi::CString;
use std::sync::atomic::{AtomicPtr, Ordering};

static NOTIFICATION_CALLBACK: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe fn nsstring(s: &str) -> *mut objc::runtime::Object {
    use objc::{class, msg_send, sel, sel_impl};
    let cs = CString::new(s).unwrap_or_default();
    let obj: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
    msg_send![obj, initWithUTF8String: cs.as_ptr()]
}

/// Callback type for notification clicks: takes the channel identifier string.
pub type NotificationCallback = Box<dyn Fn(String) + Send + Sync>;

/// Set the callback to be invoked when a notification is clicked.
/// The callback receives the channel identifier that was passed to post_notification.
pub fn set_click_callback(callback: NotificationCallback) {
    let boxed = Box::into_raw(Box::new(callback));
    NOTIFICATION_CALLBACK.store(boxed as *mut _, Ordering::SeqCst);
}

/// Returns true when running inside a proper .app bundle.
/// UNUserNotificationCenter crashes when called from a plain binary (no bundle).
fn has_bundle() -> bool {
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let bundle: *mut objc::runtime::Object = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return false;
        }
        let ident: *mut objc::runtime::Object = msg_send![bundle, bundleIdentifier];
        !ident.is_null()
    }
}

/// Request UNUserNotificationCenter authorization (alert + sound).
/// Call once at app launch. The result is asynchronous — no-op if already granted.
/// No-op when not running as a bundled app.
pub fn request_permission() {
    if !has_bundle() {
        log::trace!("Skipping notification permission: not running as a bundle");
        return;
    }
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let center: *mut objc::runtime::Object =
            msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];
        // UNAuthorizationOptionAlert = 1 << 2 = 4, UNAuthorizationOptionSound = 1 << 1 = 2
        let opts: usize = 4 | 2;
        // completionHandler: nil — we don't need the result
        let nil: *mut objc::runtime::Object = std::ptr::null_mut();
        let _: () = msg_send![center, requestAuthorizationWithOptions: opts completionHandler: nil];
    }
}

/// Post a local notification. `identifier` is used to group/remove notifications
/// per channel (e.g. UUID string or group key hex).
pub fn post_notification(title: &str, body: &str, identifier: &str) {
    if !has_bundle() {
        return;
    }
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        let center: *mut objc::runtime::Object =
            msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];

        let content: *mut objc::runtime::Object =
            msg_send![class!(UNMutableNotificationContent), new];
        let _: () = msg_send![content, setTitle: nsstring(title)];
        let _: () = msg_send![content, setBody: nsstring(body)];

        // UNTimeIntervalNotificationTrigger with 0.1 s — fires almost immediately
        let trigger: *mut objc::runtime::Object = msg_send![
            class!(UNTimeIntervalNotificationTrigger),
            triggerWithTimeInterval: 0.1_f64
            repeats: objc::runtime::NO
        ];

        let request: *mut objc::runtime::Object = msg_send![
            class!(UNNotificationRequest),
            requestWithIdentifier: nsstring(identifier)
            content: content
            trigger: trigger
        ];

        let nil: *mut objc::runtime::Object = std::ptr::null_mut();
        let _: () = msg_send![center, addNotificationRequest: request withCompletionHandler: nil];
    }
}

/// Remove all delivered and pending notifications for the given channel identifier.
pub fn remove_notifications(identifier: &str) {
    if !has_bundle() {
        return;
    }
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let center: *mut objc::runtime::Object =
            msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];
        // Build NSArray with a single identifier string
        let id_str = nsstring(identifier);
        let arr: *mut objc::runtime::Object = msg_send![class!(NSArray), arrayWithObject: id_str];
        let _: () = msg_send![center, removeDeliveredNotificationsWithIdentifiers: arr];
        let _: () = msg_send![center, removePendingNotificationRequestsWithIdentifiers: arr];
    }
}
