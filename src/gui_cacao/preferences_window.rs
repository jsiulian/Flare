use cacao::appkit::window::{Window, WindowConfig, WindowDelegate, WindowStyle};
use cacao::color::Color;
use cacao::geometry::Rect;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::text::{Font, Label, LineBreakMode};
use cacao::view::View;

use super::alert;

// NSUserDefaults keys — kept in sync with GTK GSettings keys
pub const KEY_SEND_ON_ENTER: &str = "flare.send_on_enter";
pub const KEY_NOTIFICATIONS: &str = "flare.notifications";
pub const KEY_NOTIFICATIONS_REACTIONS: &str = "flare.notifications_reactions";
pub const KEY_DOWNLOAD_IMAGES: &str = "flare.download_images";
pub const KEY_DOWNLOAD_VIDEOS: &str = "flare.download_videos";
pub const KEY_DOWNLOAD_FILES: &str = "flare.download_files";
pub const KEY_DOWNLOAD_VOICE: &str = "flare.download_voice";
pub const KEY_MESSAGES_SELECTABLE: &str = "flare.messages_selectable";

pub fn send_on_enter() -> bool {
    alert::user_default_bool(KEY_SEND_ON_ENTER).unwrap_or(true)
}

pub fn notifications_enabled() -> bool {
    alert::user_default_bool(KEY_NOTIFICATIONS).unwrap_or(true)
}

pub fn notifications_reactions_enabled() -> bool {
    alert::user_default_bool(KEY_NOTIFICATIONS_REACTIONS).unwrap_or(true)
}

pub fn download_images() -> bool {
    alert::user_default_bool(KEY_DOWNLOAD_IMAGES).unwrap_or(true)
}

pub fn download_videos() -> bool {
    alert::user_default_bool(KEY_DOWNLOAD_VIDEOS).unwrap_or(false)
}

pub fn download_files() -> bool {
    alert::user_default_bool(KEY_DOWNLOAD_FILES).unwrap_or(false)
}

pub fn download_voice() -> bool {
    alert::user_default_bool(KEY_DOWNLOAD_VOICE).unwrap_or(true)
}

pub fn messages_selectable() -> bool {
    alert::user_default_bool(KEY_MESSAGES_SELECTABLE).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Native NSSwitch wrapper (macOS 10.15+)
// ---------------------------------------------------------------------------

// Global registry for NSSwitch action callbacks.
// Each NSSwitch stores its index into this table as an associated object.
static TOGGLE_CALLBACKS: std::sync::Mutex<Vec<Box<dyn Fn(bool) + Send + Sync>>> =
    std::sync::Mutex::new(Vec::new());

/// An NSSwitch wrapped in a cacao View for autolayout integration.
struct NativeToggle {
    container: View,
    switch_ptr: usize, // raw *mut NSSwitch
}

static ASSOC_KEY: u8 = 0;

fn register_toggle_class() -> *const objc::runtime::Class {
    use std::sync::Once;
    static REGISTER: Once = Once::new();
    static mut CLASS: *const objc::runtime::Class = std::ptr::null();
    REGISTER.call_once(|| {
        use objc::declare::ClassDecl;
        use objc::runtime::{Class, Sel};

        let superclass = Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("RSTToggleTarget", superclass).unwrap();

        extern "C" fn toggled(
            this: &objc::runtime::Object,
            _sel: Sel,
            sender: *mut objc::runtime::Object,
        ) {
            unsafe {
                use objc::{msg_send, sel, sel_impl};
                use std::ffi::c_void;
                unsafe extern "C" {
                    fn objc_getAssociatedObject(
                        object: *const objc::runtime::Object,
                        key: *const c_void,
                    ) -> *mut objc::runtime::Object;
                }
                let idx_obj = objc_getAssociatedObject(
                    this as *const _,
                    &ASSOC_KEY as *const u8 as *const c_void,
                );
                if idx_obj.is_null() {
                    return;
                }
                let idx: usize = msg_send![idx_obj, unsignedIntegerValue];
                let state: i32 = msg_send![sender, state];
                let cbs = TOGGLE_CALLBACKS.lock().unwrap();
                if let Some(cb) = cbs.get(idx) {
                    cb(state == 1);
                }
            }
        }

        unsafe {
            use objc::{sel, sel_impl};
            decl.add_method(
                sel!(onToggle:),
                toggled as extern "C" fn(&objc::runtime::Object, Sel, *mut objc::runtime::Object),
            );
            CLASS = decl.register();
        }
    });
    unsafe { CLASS }
}

impl NativeToggle {
    fn new(key: &'static str, default: bool) -> Self {
        Self::new_with_action(
            alert::user_default_bool(key).unwrap_or(default),
            move |new_val| {
                alert::set_user_default_bool(key, new_val);
            },
        )
    }

    fn new_with_action<F: Fn(bool) + Send + Sync + 'static>(initial: bool, action: F) -> Self {
        use cacao::objc_access::ObjcAccess;
        use objc::{class, msg_send, sel, sel_impl};

        let container = View::new();

        // Register callback
        let idx = {
            let mut cbs = TOGGLE_CALLBACKS.lock().unwrap();
            let idx = cbs.len();
            cbs.push(Box::new(action));
            idx
        };

        unsafe {
            // Create NSSwitch
            let switch: *mut objc::runtime::Object = msg_send![class!(NSSwitch), new];
            let _: () =
                msg_send![switch, setTranslatesAutoresizingMaskIntoConstraints: objc::runtime::NO];
            let _: () = msg_send![switch, setState: if initial { 1i32 } else { 0i32 }];

            // Create target object
            let target_cls = register_toggle_class();
            let target: *mut objc::runtime::Object = msg_send![target_cls, new];

            // Store callback idx on the target via associated objects.
            use std::ffi::c_void;
            unsafe extern "C" {
                fn objc_setAssociatedObject(
                    object: *mut objc::runtime::Object,
                    key: *const c_void,
                    value: *mut objc::runtime::Object,
                    policy: usize,
                );
            }

            let idx_num: *mut objc::runtime::Object =
                msg_send![class!(NSNumber), numberWithUnsignedInteger: idx];
            objc_setAssociatedObject(
                target,
                &ASSOC_KEY as *const u8 as *const c_void,
                idx_num,
                1, // OBJC_ASSOCIATION_RETAIN_NONATOMIC
            );

            let _: () = msg_send![switch, setTarget: target];
            let _: () = msg_send![switch, setAction: sel!(onToggle:)];

            // Add NSSwitch to container
            container.with_backing_obj_mut(|view_obj| {
                let _: () = msg_send![view_obj, addSubview: switch];
                // Pin switch to all edges of container
                let la_cls = class!(NSLayoutConstraint);
                let top: *mut objc::runtime::Object = msg_send![la_cls,
                    constraintWithItem: switch
                    attribute: 3i64 // NSLayoutAttributeTop
                    relatedBy: 0i64
                    toItem: view_obj
                    attribute: 3i64
                    multiplier: 1.0f64
                    constant: 0.0f64
                ];
                let _: () = msg_send![top, setActive: objc::runtime::YES];

                let bottom: *mut objc::runtime::Object = msg_send![la_cls,
                    constraintWithItem: switch
                    attribute: 4i64 // NSLayoutAttributeBottom
                    relatedBy: 0i64
                    toItem: view_obj
                    attribute: 4i64
                    multiplier: 1.0f64
                    constant: 0.0f64
                ];
                let _: () = msg_send![bottom, setActive: objc::runtime::YES];

                let leading: *mut objc::runtime::Object = msg_send![la_cls,
                    constraintWithItem: switch
                    attribute: 5i64 // NSLayoutAttributeLeading
                    relatedBy: 0i64
                    toItem: view_obj
                    attribute: 5i64
                    multiplier: 1.0f64
                    constant: 0.0f64
                ];
                let _: () = msg_send![leading, setActive: objc::runtime::YES];

                let trailing: *mut objc::runtime::Object = msg_send![la_cls,
                    constraintWithItem: switch
                    attribute: 6i64 // NSLayoutAttributeTrailing
                    relatedBy: 0i64
                    toItem: view_obj
                    attribute: 6i64
                    multiplier: 1.0f64
                    constant: 0.0f64
                ];
                let _: () = msg_send![trailing, setActive: objc::runtime::YES];
            });

            let switch_ptr = switch as usize;
            Self {
                container,
                switch_ptr,
            }
        }
    }

    fn raw_ptr(&self) -> usize {
        self.switch_ptr
    }
}

fn set_switch_enabled(raw_ptr: usize, enabled: bool) {
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let obj = raw_ptr as *mut objc::runtime::Object;
        let _: () = msg_send![obj, setEnabled: enabled];
    }
}

// ---------------------------------------------------------------------------
// Preferences delegate
// ---------------------------------------------------------------------------

pub struct PreferencesDelegate {
    content: View,
    title: Label,

    // Section 1: Automatically Download Attachments
    dl_header: Label,
    dl_desc: Label,
    dl_images_label: Label,
    dl_images_sw: NativeToggle,
    dl_videos_label: Label,
    dl_videos_sw: NativeToggle,
    dl_files_label: Label,
    dl_files_sw: NativeToggle,
    dl_voice_label: Label,
    dl_voice_sw: NativeToggle,

    // Section 2: Notifications
    notif_header: Label,
    notif_desc: Label,
    notifications_label: Label,
    notifications_sw: NativeToggle,
    notif_reactions_label: Label,
    notif_reactions_sw: NativeToggle,
    notifications_info: Label,

    // Section 3: Mobile Compatibility
    compat_header: Label,
    compat_desc: Label,
    messages_selectable_label: Label,
    messages_selectable_sw: NativeToggle,
    send_on_enter_label: Label,
    send_on_enter_sw: NativeToggle,
}

impl Default for PreferencesDelegate {
    fn default() -> Self {
        let notif_reactions_sw = NativeToggle::new(KEY_NOTIFICATIONS_REACTIONS, true);
        let reactions_raw = notif_reactions_sw.raw_ptr();

        let notif_default = alert::user_default_bool(KEY_NOTIFICATIONS).unwrap_or(true);
        let notifications_sw = NativeToggle::new_with_action(notif_default, move |new_val| {
            alert::set_user_default_bool(KEY_NOTIFICATIONS, new_val);
            set_switch_enabled(reactions_raw, new_val);
        });

        Self {
            content: View::new(),
            title: Label::new(),

            dl_header: Label::new(),
            dl_desc: Label::new(),
            dl_images_label: Label::new(),
            dl_images_sw: NativeToggle::new(KEY_DOWNLOAD_IMAGES, true),
            dl_videos_label: Label::new(),
            dl_videos_sw: NativeToggle::new(KEY_DOWNLOAD_VIDEOS, false),
            dl_files_label: Label::new(),
            dl_files_sw: NativeToggle::new(KEY_DOWNLOAD_FILES, false),
            dl_voice_label: Label::new(),
            dl_voice_sw: NativeToggle::new(KEY_DOWNLOAD_VOICE, true),

            notif_header: Label::new(),
            notif_desc: Label::new(),
            notifications_label: Label::new(),
            notifications_sw,
            notif_reactions_label: Label::new(),
            notif_reactions_sw,
            notifications_info: Label::new(),

            compat_header: Label::new(),
            compat_desc: Label::new(),
            messages_selectable_label: Label::new(),
            messages_selectable_sw: NativeToggle::new(KEY_MESSAGES_SELECTABLE, false),
            send_on_enter_label: Label::new(),
            send_on_enter_sw: NativeToggle::new(KEY_SEND_ON_ENTER, true),
        }
    }
}

const LEADING: f64 = 24.;
const SW_W: f64 = 40.;
const ROW_GAP: f64 = 14.;
const SECTION_GAP: f64 = 28.;
const HEADER_TO_DESC: f64 = 2.;
const DESC_TO_ROW: f64 = 10.;

impl WindowDelegate for PreferencesDelegate {
    const NAME: &'static str = "PreferencesWindow";

    fn did_load(&mut self, window: Window) {
        window.set_title("Preferences");

        self.title.set_text("Preferences");
        self.title.set_font(&Font::bold_system(18.));

        // Section headers
        for (label, text) in [
            (&self.dl_header, "Automatically Download Attachments"),
            (&self.notif_header, "Notifications"),
            (&self.compat_header, "Mobile Compatibility"),
        ] {
            label.set_text(text);
            label.set_font(&Font::bold_system(13.));
        }

        // Section descriptions (gray, small)
        for (label, text) in [
            (
                &self.dl_desc,
                "Attachment types selected below will be automatically downloaded.",
            ),
            (&self.notif_desc, "Notifications for new messages."),
            (
                &self.compat_desc,
                "These options affect usability on different devices. \
                 Defaults are chosen for desktop use.",
            ),
        ] {
            label.set_text(text);
            label.set_line_break_mode(LineBreakMode::WrapWords);
            label.set_font(&Font::system(12.));
            label.set_text_color(Color::SystemGray);
        }

        // Row labels
        self.dl_images_label.set_text("Images");
        self.dl_videos_label.set_text("Videos");
        self.dl_files_label.set_text("Files");
        self.dl_voice_label.set_text("Voice Messages");
        self.notifications_label.set_text("Send Notifications");
        self.notif_reactions_label
            .set_text("Send Notifications on Reactions");
        self.messages_selectable_label
            .set_text("Selectable Message Text");
        self.send_on_enter_label
            .set_text("Press \u{201C}Enter\u{201D} to Send Message");

        self.notifications_info.set_text(
            "For full notification control, open System Settings \u{2192} \
             Notifications \u{2192} Flare.",
        );
        self.notifications_info
            .set_line_break_mode(LineBreakMode::WrapWords);
        self.notifications_info.set_font(&Font::system(11.));
        self.notifications_info.set_text_color(Color::SystemGray);

        // Initial sensitivity: disable reactions toggle when notifications off
        if !alert::user_default_bool(KEY_NOTIFICATIONS).unwrap_or(true) {
            set_switch_enabled(self.notif_reactions_sw.raw_ptr(), false);
        }

        // Add all subviews
        for label in [
            &self.title,
            &self.dl_header,
            &self.dl_desc,
            &self.dl_images_label,
            &self.dl_videos_label,
            &self.dl_files_label,
            &self.dl_voice_label,
            &self.notif_header,
            &self.notif_desc,
            &self.notifications_label,
            &self.notif_reactions_label,
            &self.notifications_info,
            &self.compat_header,
            &self.compat_desc,
            &self.messages_selectable_label,
            &self.send_on_enter_label,
        ] {
            self.content.add_subview(label);
        }
        for sw in [
            &self.dl_images_sw,
            &self.dl_videos_sw,
            &self.dl_files_sw,
            &self.dl_voice_sw,
            &self.notifications_sw,
            &self.notif_reactions_sw,
            &self.messages_selectable_sw,
            &self.send_on_enter_sw,
        ] {
            self.content.add_subview(&sw.container);
        }

        // Layout helper: one toggle row anchored to a top constraint
        macro_rules! row {
            ($label:expr, $sw:expr, $top_anchor:expr, $offset:expr) => {
                vec![
                    $label.top.constraint_equal_to(&$top_anchor).offset($offset),
                    $label
                        .leading
                        .constraint_equal_to(&self.content.leading)
                        .offset(LEADING),
                    $label
                        .trailing
                        .constraint_less_than_or_equal_to(&$sw.container.leading)
                        .offset(-8.),
                    $sw.container.center_y.constraint_equal_to(&$label.center_y),
                    $sw.container
                        .trailing
                        .constraint_equal_to(&self.content.trailing)
                        .offset(-LEADING),
                    $sw.container.width.constraint_equal_to_constant(SW_W),
                ]
            };
        }

        let mut c = vec![
            // Title
            self.title
                .top
                .constraint_equal_to(&self.content.top)
                .offset(LEADING),
            self.title
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.title
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-LEADING),
            // -- Downloads --
            self.dl_header
                .top
                .constraint_equal_to(&self.title.bottom)
                .offset(SECTION_GAP),
            self.dl_header
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.dl_desc
                .top
                .constraint_equal_to(&self.dl_header.bottom)
                .offset(HEADER_TO_DESC),
            self.dl_desc
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.dl_desc
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-LEADING),
        ];
        c.extend(row!(
            self.dl_images_label,
            self.dl_images_sw,
            self.dl_desc.bottom,
            DESC_TO_ROW
        ));
        c.extend(row!(
            self.dl_videos_label,
            self.dl_videos_sw,
            self.dl_images_label.bottom,
            ROW_GAP
        ));
        c.extend(row!(
            self.dl_files_label,
            self.dl_files_sw,
            self.dl_videos_label.bottom,
            ROW_GAP
        ));
        c.extend(row!(
            self.dl_voice_label,
            self.dl_voice_sw,
            self.dl_files_label.bottom,
            ROW_GAP
        ));

        // -- Notifications --
        c.extend(vec![
            self.notif_header
                .top
                .constraint_equal_to(&self.dl_voice_label.bottom)
                .offset(SECTION_GAP),
            self.notif_header
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.notif_desc
                .top
                .constraint_equal_to(&self.notif_header.bottom)
                .offset(HEADER_TO_DESC),
            self.notif_desc
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.notif_desc
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-LEADING),
        ]);
        c.extend(row!(
            self.notifications_label,
            self.notifications_sw,
            self.notif_desc.bottom,
            DESC_TO_ROW
        ));
        c.extend(row!(
            self.notif_reactions_label,
            self.notif_reactions_sw,
            self.notifications_label.bottom,
            ROW_GAP
        ));
        c.extend(vec![
            self.notifications_info
                .top
                .constraint_equal_to(&self.notif_reactions_label.bottom)
                .offset(8.),
            self.notifications_info
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.notifications_info
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-LEADING),
        ]);

        // -- Mobile Compatibility --
        c.extend(vec![
            self.compat_header
                .top
                .constraint_equal_to(&self.notifications_info.bottom)
                .offset(SECTION_GAP),
            self.compat_header
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.compat_desc
                .top
                .constraint_equal_to(&self.compat_header.bottom)
                .offset(HEADER_TO_DESC),
            self.compat_desc
                .leading
                .constraint_equal_to(&self.content.leading)
                .offset(LEADING),
            self.compat_desc
                .trailing
                .constraint_equal_to(&self.content.trailing)
                .offset(-LEADING),
        ]);
        c.extend(row!(
            self.messages_selectable_label,
            self.messages_selectable_sw,
            self.compat_desc.bottom,
            DESC_TO_ROW
        ));
        c.extend(row!(
            self.send_on_enter_label,
            self.send_on_enter_sw,
            self.messages_selectable_label.bottom,
            ROW_GAP
        ));

        LayoutConstraint::activate(&c);
        window.set_content_view(&self.content);
    }
}

pub struct PreferencesWindow(pub Option<Window<PreferencesDelegate>>);

impl PreferencesWindow {
    pub fn show(&self) {
        if let Some(ref w) = self.0 {
            w.show();
        }
    }
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        let mut config = WindowConfig::default();
        config.set_styles(&[
            WindowStyle::Titled,
            WindowStyle::Closable,
            WindowStyle::Miniaturizable,
        ]);
        config.initial_dimensions = Rect::new(0.0, 0.0, 440.0, 560.0);
        Self(Some(Window::with(config, PreferencesDelegate::default())))
    }
}
