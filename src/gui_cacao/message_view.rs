use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};

use cacao::appkit::App;
use cacao::button::Button;
use cacao::color::Color;
use cacao::image::{Image, ImageView};

/// Stores the last right-clicked row so it survives menu closure.
pub static LAST_CLICKED_ROW: AtomicIsize = AtomicIsize::new(-1);

/// Stores a pointer to the NSTableView so reaction buttons can look up their row.
static TABLE_VIEW_PTR: AtomicUsize = AtomicUsize::new(0);
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::objc_access::ObjcAccess;
use cacao::text::{Font, Label, LineBreakMode, TextAlign};
use cacao::view::{View, ViewDelegate};

// Hover tracking for react buttons

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}
unsafe impl objc::Encode for NSRect {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRange {
    location: usize,
    length: usize,
}
unsafe impl objc::Encode for NSRange {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{_NSRange=QQ}") }
    }
}

fn register_hover_owner_class() -> *const objc::runtime::Class {
    static ONCE: std::sync::Once = std::sync::Once::new();
    static mut CLS: *const objc::runtime::Class = std::ptr::null();

    extern "C" fn mouse_entered(
        this: &objc::runtime::Object,
        _cmd: objc::runtime::Sel,
        _event: *mut objc::runtime::Object,
    ) {
        unsafe {
            use objc::{msg_send, sel, sel_impl};
            let ptr: usize = *this.get_ivar::<usize>("rstBtnPtr");
            if ptr != 0 {
                let btn = ptr as *mut objc::runtime::Object;
                let _: () = msg_send![btn, setHidden: objc::runtime::NO];
            }
        }
    }
    extern "C" fn mouse_exited(
        this: &objc::runtime::Object,
        _cmd: objc::runtime::Sel,
        _event: *mut objc::runtime::Object,
    ) {
        unsafe {
            use objc::{msg_send, sel, sel_impl};
            let ptr: usize = *this.get_ivar::<usize>("rstBtnPtr");
            if ptr != 0 {
                let btn = ptr as *mut objc::runtime::Object;
                let _: () = msg_send![btn, setHidden: objc::runtime::YES];
            }
        }
    }

    ONCE.call_once(|| unsafe {
        use objc::{class, sel, sel_impl};
        let superclass = class!(NSObject);
        let mut decl = objc::declare::ClassDecl::new("RSTHoverOwner", superclass).unwrap();
        decl.add_ivar::<usize>("rstBtnPtr");
        decl.add_method(
            sel!(mouseEntered:),
            mouse_entered
                as extern "C" fn(
                    &objc::runtime::Object,
                    objc::runtime::Sel,
                    *mut objc::runtime::Object,
                ),
        );
        decl.add_method(
            sel!(mouseExited:),
            mouse_exited
                as extern "C" fn(
                    &objc::runtime::Object,
                    objc::runtime::Sel,
                    *mut objc::runtime::Object,
                ),
        );
        CLS = decl.register();
    });

    unsafe { CLS }
}

/// Install an NSTrackingArea on `row_view` that shows/hides `button` on hover.
/// The owner object is leaked intentionally (lives as long as the row).
fn install_hover_tracking(row_view: &View, button: &Button) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let btn_ptr: usize = button.objc.get(|obj| obj as *const _ as usize);

    row_view.with_backing_obj_mut(|view_obj| unsafe {
        // Create the hover owner
        let cls = register_hover_owner_class();
        let owner: *mut Object = msg_send![cls, alloc];
        let owner: *mut Object = msg_send![owner, init];
        (*owner).set_ivar("rstBtnPtr", btn_ptr);

        // NSTrackingMouseEnteredAndExited | NSTrackingActiveInActiveApp | NSTrackingInVisibleRect
        let opts: usize = 0x01 | 0x20 | 0x200;
        let zero_rect = NSRect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        };
        let nil: *mut Object = std::ptr::null_mut();
        let ta: *mut Object = msg_send![class!(NSTrackingArea), alloc];
        let ta: *mut Object =
            msg_send![ta, initWithRect: zero_rect options: opts owner: owner userInfo: nil];
        let _: () = msg_send![view_obj, addTrackingArea: ta];
        // owner and ta are intentionally leaked — they live with the row view
    });
}

use crate::core::message::CoreMessage;

use super::app::{AppMessage, FlareApp};
use super::menu_action::attach_message_context_menu;
use super::preferences_window;

const MSG_OUT_ROW: &str = "MsgOutCell";
const MSG_IN_ROW: &str = "MsgInCell";
const DATE_ROW: &str = "DateDividerCell";

/// Aggregated reactions for a message: list of "emoji sender_name" strings.
pub type ReactionList = Vec<String>;

#[derive(Debug, Clone)]
pub enum MessageItem {
    Outgoing(CoreMessage, ReactionList),
    Incoming(CoreMessage, ReactionList),
    Call(CoreMessage),
    DateDivider(String),
}

impl MessageItem {
    pub fn timestamp(&self) -> u64 {
        match self {
            MessageItem::Outgoing(m, _) | MessageItem::Incoming(m, _) | MessageItem::Call(m) => {
                m.timestamp
            }
            MessageItem::DateDivider(_) => 0,
        }
    }
}

fn date_label_for_timestamp(ts: u64) -> String {
    let secs = ts / 1000;
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| {
            let now = chrono::Utc::now();
            if dt.date_naive() == now.date_naive() {
                "Today".to_string()
            } else {
                dt.format("%B %-d, %Y").to_string()
            }
        })
        .unwrap_or_default()
}

pub fn build_message_items(messages: Vec<CoreMessage>) -> Vec<MessageItem> {
    use std::collections::HashMap;

    // Build a map: target_ts -> Vec<(sender_name, emoji)>
    // Each sender can have at most one reaction; newer reactions replace older ones.
    let mut reaction_map: HashMap<u64, Vec<(String, String)>> = HashMap::new();
    for msg in messages.iter().filter(|m| m.is_reaction) {
        if let Some(ts) = msg.reaction_target_ts {
            let sender = msg.sender_name.clone();
            let entry = reaction_map.entry(ts).or_default();
            if msg.reaction_remove {
                entry.retain(|(s, _)| s != &sender);
            } else {
                let emoji = msg.body.as_deref().unwrap_or("").to_string();
                if let Some(existing) = entry.iter_mut().find(|(s, _)| s == &sender) {
                    existing.1 = emoji;
                } else {
                    entry.push((sender, emoji));
                }
            }
        }
    }

    let mut items: Vec<MessageItem> = Vec::new();
    let mut last_date: Option<chrono::NaiveDate> = None;

    for msg in messages {
        if msg.is_reaction {
            continue;
        }

        let secs = msg.timestamp / 1000;
        let date = chrono::DateTime::from_timestamp(secs as i64, 0).map(|dt| dt.date_naive());
        if let Some(d) = date {
            if last_date.map_or(true, |prev| prev != d) {
                items.push(MessageItem::DateDivider(date_label_for_timestamp(
                    msg.timestamp,
                )));
                last_date = Some(d);
            }
        }

        let reactions: ReactionList = reaction_map
            .remove(&msg.timestamp)
            .map(|v| {
                v.into_iter()
                    .map(|(sender, emoji)| format!("{} {}", emoji, sender))
                    .collect()
            })
            .unwrap_or_default();

        if msg.is_call {
            items.push(MessageItem::Call(msg));
        } else if msg.is_outgoing {
            items.push(MessageItem::Outgoing(msg, reactions));
        } else {
            items.push(MessageItem::Incoming(msg, reactions));
        }
    }

    items
}

/// Check if the NSTableView is scrolled to the bottom (within a small threshold).
fn is_scrolled_to_bottom(view: &ListView) -> bool {
    use objc::{class, msg_send, sel, sel_impl};
    let table = view
        .objc
        .get(|obj| obj as *const _ as *mut objc::runtime::Object);
    unsafe {
        let enclosing_scroll: *mut objc::runtime::Object = msg_send![table, enclosingScrollView];
        if enclosing_scroll.is_null() {
            return true;
        }
        let clip_view: *mut objc::runtime::Object = msg_send![enclosing_scroll, contentView];
        if clip_view.is_null() {
            return true;
        }
        let document_view: *mut objc::runtime::Object = msg_send![enclosing_scroll, documentView];
        if document_view.is_null() {
            return true;
        }
        let doc_rect: NSRect = msg_send![document_view, frame];
        let clip_rect: NSRect = msg_send![clip_view, frame];
        let clip_bounds: NSRect = msg_send![clip_view, bounds];
        let doc_height = doc_rect.h;
        let clip_height = clip_rect.h;
        let content_offset = clip_bounds.y;
        // Consider "at bottom" if within 50 pixels of the bottom
        (content_offset + clip_height >= doc_height - 50.0)
    }
}

/// Scroll an NSTableView to its last row, only if already scrolled to bottom or forced.
fn scroll_to_bottom(view: &ListView, force: bool) {
    use objc::{msg_send, sel, sel_impl};
    let table = view
        .objc
        .get(|obj| obj as *const _ as *mut objc::runtime::Object);

    // Only scroll if forced (e.g., new outgoing message) or user is already at bottom
    if !force && !is_scrolled_to_bottom(view) {
        return;
    }

    unsafe {
        let count: usize = msg_send![table, numberOfRows];
        if count > 0 {
            let _: () = msg_send![table, scrollRowToVisible: (count - 1) as isize];
        }
    }
}

/// Scroll to a specific row index.
fn scroll_to_message_index(view: &ListView, index: usize) {
    use objc::{msg_send, sel, sel_impl};
    let table = view
        .objc
        .get(|obj| obj as *const _ as *mut objc::runtime::Object);

    unsafe {
        let count: usize = msg_send![table, numberOfRows];
        if count > 0 && index < count {
            let _: () = msg_send![table, scrollRowToVisible: index as isize];
        }
    }
}

/// Tell NSTableView to recalculate row heights for all rows.
fn note_height_changed(table: *mut objc::runtime::Object, count: usize) {
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let range = NSRange {
            location: 0,
            length: count,
        };
        let index_set: *mut objc::runtime::Object =
            msg_send![class!(NSIndexSet), indexSetWithIndexesInRange: range];
        let _: () = msg_send![table, noteHeightOfRowsWithIndexesChanged: index_set];
    }
}

/// Insert new row at end without full reload - for smooth animation.
fn insert_new_row(view: &ListView) {
    use objc::{class, msg_send, sel, sel_impl};
    let ptr = view
        .objc
        .get(|obj| obj as *const _ as *mut objc::runtime::Object);
    unsafe {
        let count: usize = msg_send![ptr, numberOfRows];
        if count == 0 {
            return;
        }
        let idx = count - 1;
        let set: *mut objc::runtime::Object = msg_send![class!(NSIndexSet), indexSetWithIndex: idx];
        let _: () = msg_send![ptr, insertRowsAtIndexes: set withAnimation: 1];
    }
}

/// Reload the ListView without holding a borrow on its ObjcProperty.
/// synchronously re-enters `item_for` → `dequeue` which calls `get` (borrow)
/// on the same RefCell, causing a panic. We avoid this by extracting the
/// raw pointer first, releasing the borrow, then sending the message.
pub fn reload_listview(view: &ListView) {
    use objc::{msg_send, sel, sel_impl};
    let ptr = view
        .objc
        .get(|obj| obj as *const _ as *mut objc::runtime::Object);
    unsafe {
        let _: () = msg_send![ptr, reloadData];
    }
}

// Receipt status tracking

/// 0 = sent only, 1 = delivered, 2 = read/viewed
static RECEIPT_STATUS: once_cell::sync::Lazy<Mutex<HashMap<u64, u8>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub fn update_receipt_status(timestamps: &[u64], is_read: bool) {
    if let Ok(mut map) = RECEIPT_STATUS.lock() {
        let level: u8 = if is_read { 2 } else { 1 };
        for &ts in timestamps {
            let entry = map.entry(ts).or_insert(0);
            if level > *entry {
                *entry = level;
            }
        }
    }
}

fn receipt_indicator(ts: u64) -> &'static str {
    match RECEIPT_STATUS.lock().ok().and_then(|m| m.get(&ts).copied()) {
        Some(2) => "✓✓",
        Some(1) => "✓",
        _ => "",
    }
}

// Per-row attachment index for double-click

use std::sync::Mutex;
/// Maps row index → attachment pointer. Rebuilt whenever items change.
static ROW_ATTACHMENTS: Mutex<Vec<Option<libsignal_service::proto::AttachmentPointer>>> =
    Mutex::new(Vec::new());

/// Maps row index → first URL in the message body (if any).
static ROW_URLS: Mutex<Vec<Option<String>>> = Mutex::new(Vec::new());

fn rebuild_row_attachments(items: &[MessageItem]) {
    if let Ok(mut guard) = ROW_ATTACHMENTS.lock() {
        *guard = items
            .iter()
            .map(|item| match item {
                MessageItem::Outgoing(m, _)
                | MessageItem::Incoming(m, _)
                | MessageItem::Call(m) => m.attachments.first().map(|a| {
                    let mut pointer = libsignal_service::proto::AttachmentPointer::default();
                    pointer.content_type = a.content_type.clone();
                    pointer.file_name = a.file_name.clone();
                    pointer.size = a.size;
                    pointer.width = a.width;
                    pointer.height = a.height;
                    pointer.blur_hash = a.blur_hash.clone();
                    pointer.digest = a.digest.clone();
                    pointer
                }),
                MessageItem::DateDivider(_) => None,
            })
            .collect();
    }
    if let Ok(mut guard) = ROW_URLS.lock() {
        *guard = items
            .iter()
            .map(|item| match item {
                MessageItem::Outgoing(m, _)
                | MessageItem::Incoming(m, _)
                | MessageItem::Call(m) => m.body.as_deref().and_then(first_url_from_text),
                MessageItem::DateDivider(_) => None,
            })
            .collect();
    }
}

/// Set up NSTableView's action (single-click opens URLs) and doubleAction (opens attachments).
fn setup_table_double_action(table: *mut objc::runtime::Object) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    static ONCE: std::sync::Once = std::sync::Once::new();
    static TARGET_PTR: AtomicUsize = AtomicUsize::new(0);

    ONCE.call_once(|| unsafe {
        let sup = class!(NSObject);
        let mut decl = objc::declare::ClassDecl::new("RSTTableDblClick", sup).unwrap();

        extern "C" fn on_row_click(_this: &Object, _sel: objc::runtime::Sel, sender: *mut Object) {
            unsafe {
                use objc::{msg_send, sel, sel_impl};
                let row: isize = msg_send![sender, clickedRow];
                if row < 0 {
                    return;
                }
                if let Ok(urls) = ROW_URLS.lock() {
                    if let Some(Some(url)) = urls.get(row as usize) {
                        open_url_via_workspace(url);
                    }
                }
            }
        }

        extern "C" fn on_row_double_click(
            _this: &Object,
            _sel: objc::runtime::Sel,
            sender: *mut Object,
        ) {
            unsafe {
                use objc::{msg_send, sel, sel_impl};
                let row: isize = msg_send![sender, clickedRow];
                if row < 0 {
                    return;
                }
                if let Ok(attachments) = ROW_ATTACHMENTS.lock() {
                    if let Some(Some(pointer)) = attachments.get(row as usize) {
                        App::<FlareApp, AppMessage>::dispatch_main(AppMessage::OpenAttachment(
                            pointer.clone(),
                        ));
                    }
                }
            }
        }

        use objc::{sel, sel_impl};
        decl.add_method(
            sel!(onRowClick:),
            on_row_click as extern "C" fn(&Object, objc::runtime::Sel, *mut Object),
        );
        decl.add_method(
            sel!(onRowDoubleClick:),
            on_row_double_click as extern "C" fn(&Object, objc::runtime::Sel, *mut Object),
        );
        let cls = decl.register();
        let target: *mut Object = msg_send![cls, new];
        // Manually retain so it outlives the table's weak reference
        let _: *mut Object = msg_send![target, retain];
        TARGET_PTR.store(target as usize, Ordering::Relaxed);
    });

    unsafe {
        let target = TARGET_PTR.load(Ordering::Relaxed) as *mut Object;
        let _: () = msg_send![table, setAction: sel!(onRowClick:)];
        let _: () = msg_send![table, setDoubleAction: sel!(onRowDoubleClick:)];
        let _: () = msg_send![table, setTarget: target];
    }
}

/// Set a small SF Symbol smiley icon on a reaction button.
fn set_react_button_icon(btn: &Button) {
    use std::ffi::CString;
    btn.objc.get(|obj| unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let obj = obj as *const _ as *mut objc::runtime::Object;
        let cs = CString::new("face.smiling").unwrap_or_default();
        let ns_name: *mut objc::runtime::Object = {
            let s: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
            msg_send![s, initWithUTF8String: cs.as_ptr()]
        };
        let nil: *mut objc::runtime::Object = std::ptr::null_mut();
        let image: *mut objc::runtime::Object = msg_send![
            class!(NSImage),
            imageWithSystemSymbolName: ns_name
            accessibilityDescription: nil
        ];
        if !image.is_null() {
            let _: () = msg_send![obj, setImage: image];
            let _: () = msg_send![obj, setBordered: objc::runtime::NO];
            let _: () = msg_send![obj, setImageScaling: 3i32]; // NSImageScaleProportionallyDown
        }
    });
}

/// Pointer to a hidden NSTextView subclass used to capture emoji picker selections.
static EMOJI_CAPTURE_PTR: AtomicUsize = AtomicUsize::new(0);

fn register_emoji_capture_class() -> *const objc::runtime::Class {
    static ONCE: std::sync::Once = std::sync::Once::new();
    static mut CLS: *const objc::runtime::Class = std::ptr::null();

    extern "C" fn insert_text(
        this: &objc::runtime::Object,
        _cmd: objc::runtime::Sel,
        string: *mut objc::runtime::Object,
        _range: NSRange,
    ) {
        unsafe {
            use objc::{class, msg_send, sel, sel_impl};
            // Extract NSString value (handles both NSString and NSAttributedString)
            let is_attr: bool = msg_send![string, isKindOfClass: class!(NSAttributedString)];
            let ns_str: *mut objc::runtime::Object = if is_attr {
                msg_send![string, string]
            } else {
                string
            };
            let bytes: *const std::os::raw::c_char = msg_send![ns_str, UTF8String];
            if bytes.is_null() {
                return;
            }
            let emoji = std::ffi::CStr::from_ptr(bytes)
                .to_string_lossy()
                .into_owned();
            if !emoji.is_empty() {
                App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ReactToMessage(emoji));
            }
            // Resign first responder so we don't capture future keyboard input
            let window: *mut objc::runtime::Object = msg_send![this, window];
            if !window.is_null() {
                let _: bool = msg_send![window, makeFirstResponder: std::ptr::null::<objc::runtime::Object>()];
            }
        }
    }

    ONCE.call_once(|| unsafe {
        use objc::{class, sel, sel_impl};
        let superclass = class!(NSTextView);
        let mut decl = objc::declare::ClassDecl::new("RSTEmojiCaptureView", superclass).unwrap();
        decl.add_method(
            sel!(insertText:replacementRange:),
            insert_text
                as extern "C" fn(
                    &objc::runtime::Object,
                    objc::runtime::Sel,
                    *mut objc::runtime::Object,
                    NSRange,
                ),
        );
        CLS = decl.register();
    });

    unsafe { CLS }
}

/// Get or create the hidden emoji capture view, added as a 1×1 subview of the table's scroll view.
unsafe fn get_emoji_capture_view() -> *mut objc::runtime::Object {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let ptr = EMOJI_CAPTURE_PTR.load(Ordering::Relaxed);
    if ptr != 0 {
        return ptr as *mut Object;
    }

    let table = TABLE_VIEW_PTR.load(Ordering::Relaxed) as *mut Object;
    if table.is_null() {
        return std::ptr::null_mut();
    }

    // Get scroll view parent
    let scroll: *mut Object = msg_send![table, enclosingScrollView];
    let parent: *mut Object = if scroll.is_null() { table } else { scroll };

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    unsafe impl objc::Encode for NSRect {
        fn encode() -> objc::Encoding {
            unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
        }
    }

    let cls = register_emoji_capture_class();
    let view: *mut Object = msg_send![cls, alloc];
    let frame = NSRect {
        x: -10.0,
        y: -10.0,
        w: 1.0,
        h: 1.0,
    };
    let view: *mut Object = msg_send![view, initWithFrame: frame];
    let _: () = msg_send![parent, addSubview: view];

    EMOJI_CAPTURE_PTR.store(view as usize, Ordering::Relaxed);
    view
}

/// Store the row and open the OS emoji picker, routing the selection to that row.
fn show_emoji_picker_for_row(sender: *mut objc::runtime::Object) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let table = TABLE_VIEW_PTR.load(Ordering::Relaxed) as *mut Object;
        if table.is_null() {
            return;
        }
        let row: isize = msg_send![table, rowForView: sender];
        if row < 0 {
            return;
        }
        LAST_CLICKED_ROW.store(row, Ordering::Relaxed);

        let capture = get_emoji_capture_view();
        if capture.is_null() {
            return;
        }

        let window: *mut Object = msg_send![table, window];
        if window.is_null() {
            return;
        }
        let _: bool = msg_send![window, makeFirstResponder: capture];

        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, orderFrontCharacterPalette: std::ptr::null::<Object>()];
    }
}

fn set_bubble_style(bubble: &View, outgoing: bool) {
    let (r, g, b, a) = if outgoing {
        preferences_window::bubble_outgoing_color()
    } else {
        preferences_window::bubble_incoming_color()
    };
    let color = Color::rgba(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    );
    bubble.set_background_color(color);

    use cacao::foundation::YES;
    use cacao::objc_access::ObjcAccess;
    bubble.with_backing_obj_mut(|obj| unsafe {
        use objc::{msg_send, sel, sel_impl};
        let layer: *mut objc::runtime::Object = msg_send![obj, layer];
        if !layer.is_null() {
            let _: () = msg_send![layer, setCornerRadius: 16.0];
            let _: () = msg_send![layer, setMasksToBounds: YES];
        }
    });
}

/// Set the constant on a stored LayoutConstraint (the image height/width constraint).
fn set_constraint_constant(constraint: &LayoutConstraint, value: f64) {
    unsafe {
        use objc::{msg_send, sel, sel_impl};
        let _: () = msg_send![&*constraint.constraint, setConstant: value];
    }
}

/// Set image scaling to NSImageScaleProportionallyDown so images fit without cropping.
fn set_image_scaling(view: &ImageView) {
    use cacao::objc_access::ObjcAccess;
    view.with_backing_obj_mut(|obj| unsafe {
        use objc::{msg_send, sel, sel_impl};
        // NSImageScaleProportionallyUpOrDown = 3
        let _: () = msg_send![obj, setImageScaling: 3i32];
    });
}

/// Find URL byte ranges in `text`. Returns (start, end) byte offset pairs.
fn find_url_byte_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut search_from = 0;
    while search_from < text.len() {
        let slice = &text[search_from..];
        let found = ["https://", "http://", "www."]
            .iter()
            .filter_map(|prefix| slice.find(prefix).map(|i| i + search_from))
            .min();
        let Some(start) = found else { break };
        // URL ends at the next whitespace character
        let end = text[start..]
            .find(|c: char| c.is_whitespace())
            .map_or(text.len(), |i| start + i);
        ranges.push((start, end));
        search_from = end;
    }
    ranges
}

/// Convert a byte offset in a UTF-8 string to the equivalent UTF-16 code-unit count.
fn byte_to_utf16_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())].encode_utf16().count()
}

/// Apply text to a Label, detecting URLs and making them clickable links.
/// Uses pure-Rust URL detection to avoid ObjC exceptions. Falls back to
/// plain `set_text` when no URLs are present.
fn apply_body_text(label: &Label, text: &str, outgoing: bool) {
    let url_ranges = find_url_byte_ranges(text);
    if url_ranges.is_empty() {
        label.set_text(text);
        return;
    }

    use cacao::objc_access::ObjcAccess;
    label.with_backing_obj_mut(|obj| unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        use std::ffi::CString;

        let cs = match CString::new(text) {
            Ok(c) => c,
            Err(_) => { label.set_text(text); return; }
        };
        let ns_str: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: cs.as_ptr()];
        if ns_str.is_null() {
            label.set_text(text);
            return;
        }

        let alloc: *mut objc::runtime::Object = msg_send![class!(NSMutableAttributedString), alloc];
        let attr_str: *mut objc::runtime::Object = msg_send![alloc, initWithString: ns_str];
        if attr_str.is_null() {
            label.set_text(text);
            return;
        }

        // Underline attribute key/value (used for outgoing bubbles where NSLink would be invisible)
        let underline_key_cs = CString::new("NSUnderline").unwrap_or_default();
        let underline_key: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: underline_key_cs.as_ptr()];
        let underline_val: *mut objc::runtime::Object =
            msg_send![class!(NSNumber), numberWithInt: 1i32];

        // NSLink key (for incoming: renders blue + clickable)
        let link_key_cs = CString::new("NSLink").unwrap_or_default();
        let link_key: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: link_key_cs.as_ptr()];

        for (byte_start, byte_end) in &url_ranges {
            let url_slice = &text[*byte_start..*byte_end];
            let url_with_scheme: std::borrow::Cow<str> = if url_slice.starts_with("www.") {
                format!("https://{}", url_slice).into()
            } else {
                url_slice.into()
            };

            let utf16_start = byte_to_utf16_offset(text, *byte_start);
            let utf16_len = url_slice.encode_utf16().count();
            let range = NSRange { location: utf16_start, length: utf16_len };

            if outgoing {
                // On blue bubble: keep text white, just underline the URL
                let _: () = msg_send![attr_str, addAttribute: underline_key value: underline_val range: range];
            } else {
                // On light bubble: use NSLink so the URL appears blue and is clickable
                let url_cs = match CString::new(url_with_scheme.as_ref()) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let url_ns: *mut objc::runtime::Object =
                    msg_send![class!(NSString), stringWithUTF8String: url_cs.as_ptr()];
                if url_ns.is_null() { continue; }
                let nsurl: *mut objc::runtime::Object =
                    msg_send![class!(NSURL), URLWithString: url_ns];
                if nsurl.is_null() { continue; }
                let _: () = msg_send![attr_str, addAttribute: link_key value: nsurl range: range];
            }
        }

        let _: () = msg_send![obj, setAttributedStringValue: attr_str];
    });
}

/// Apply the "Selectable Message Text" preference to a label by toggling the
/// underlying NSTextField's selectable state.
fn apply_label_selectable(label: &Label) {
    use cacao::objc_access::ObjcAccess;
    let selectable = preferences_window::messages_selectable();
    label.with_backing_obj_mut(|obj| unsafe {
        use objc::{msg_send, sel, sel_impl};
        let field: *mut objc::runtime::Object = obj as *const _ as *mut objc::runtime::Object;
        let on = if selectable {
            objc::runtime::YES
        } else {
            objc::runtime::NO
        };
        // NSTextField: allow selection and editing attributes when selectable is enabled.
        let _: () = msg_send![field, setSelectable: on];
        let _: () = msg_send![field, setAllowsEditingTextAttributes: on];
    });
}

/// Extract the first URL from text using find_url_byte_ranges, prepending https:// for www. links.
fn first_url_from_text(text: &str) -> Option<String> {
    let ranges = find_url_byte_ranges(text);
    ranges.first().map(|(start, end)| {
        let slice = &text[*start..*end];
        if slice.starts_with("www.") {
            format!("https://{}", slice)
        } else {
            slice.to_string()
        }
    })
}

/// Open a URL string via NSWorkspace.
fn open_url_via_workspace(url: &str) {
    use std::ffi::CString;
    let Ok(cs) = CString::new(url) else { return };
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};
        let ns_str: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: cs.as_ptr()];
        if ns_str.is_null() {
            return;
        }
        let nsurl: *mut objc::runtime::Object = msg_send![class!(NSURL), URLWithString: ns_str];
        if nsurl.is_null() {
            return;
        }
        let workspace: *mut objc::runtime::Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let _: bool = msg_send![workspace, openURL: nsurl];
    }
}

fn format_time(ts: u64) -> String {
    let secs = ts / 1000;
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_default()
}

// Outgoing message row (blue bubble, right-aligned)

#[derive(Debug)]
pub struct OutgoingRow {
    pub bubble: View,
    pub body_label: Label,
    pub image_view: ImageView,
    pub video_view: View,
    pub audio_label: Label,
    pub file_label: Label,
    pub delivery_label: Label,
    pub reactions_label: Label,
    pub react_button: Button,
    image_height: Option<LayoutConstraint>,
    image_width: Option<LayoutConstraint>,
    video_height: Option<LayoutConstraint>,
    audio_height: Option<LayoutConstraint>,
    file_height: Option<LayoutConstraint>,
    bubble_min_height: Option<LayoutConstraint>,
    bubble_min_width: Option<LayoutConstraint>,
}

impl Default for OutgoingRow {
    fn default() -> Self {
        Self {
            bubble: View::new(),
            body_label: Label::new(),
            image_view: ImageView::new(),
            video_view: View::new(),
            audio_label: Label::new(),
            file_label: Label::new(),
            delivery_label: Label::new(),
            reactions_label: Label::new(),
            react_button: Button::new(""),
            image_height: None,
            image_width: None,
            video_height: None,
            audio_height: None,
            file_height: None,
            bubble_min_height: None,
            bubble_min_width: None,
        }
    }
}

impl OutgoingRow {
    pub fn configure_with(&mut self, msg: &CoreMessage, reactions: &[String]) {
        let body_text = if let Some(ref q) = msg.quote {
            let quoted = q.text.as_deref().unwrap_or("[attachment]");
            let truncated: String = quoted.chars().take(60).collect();
            let ellipsis = if quoted.chars().count() > 60 {
                "…"
            } else {
                ""
            };
            format!(
                "↩ {}{}\n{}",
                truncated,
                ellipsis,
                msg.body.as_deref().unwrap_or("")
            )
        } else {
            msg.body.as_deref().unwrap_or("").to_string()
        };
        apply_body_text(&self.body_label, &body_text, true);
        let indicator = receipt_indicator(msg.timestamp);
        let time_str = format_time(msg.timestamp);
        self.delivery_label.set_text(if indicator.is_empty() {
            time_str
        } else {
            format!("{} {}", time_str, indicator)
        });
        self.react_button.set_hidden(true);
        if let (Some(hc), Some(wc)) = (&self.image_height, &self.image_width) {
            if let Some(ref path) = msg.image_path {
                let image = Image::with_contents_of_file(&path.to_string_lossy());
                // Always set the image and show the view; NSImage will handle decode failures.
                self.image_view.set_image(&image);
                // Make the preview clearly visible: tall bubble plus wide image.
                set_constraint_constant(hc, 220.0);
                set_constraint_constant(wc, 380.0);
                self.image_view.set_hidden(false);
            } else {
                set_constraint_constant(hc, 0.0);
                set_constraint_constant(wc, 0.0);
                self.image_view.set_hidden(true);
            }
        }
        // Handle video attachments - show a placeholder with play icon
        if msg.video_path.is_some() {
            if let Some(vh) = &self.video_height {
                self.video_view.set_hidden(false);
                set_constraint_constant(vh, 60.0);
                // Add play icon to video view
                self.video_view.set_background_color(Color::SystemGray);
                self.video_view.layer.set_corner_radius(8.0);
            }
        } else if let Some(vh) = &self.video_height {
            self.video_view.set_hidden(true);
            set_constraint_constant(vh, 0.0);
        }
        // Handle audio attachments - show a play button label
        if msg.audio_path.is_some() {
            if let Some(ah) = &self.audio_height {
                self.audio_label.set_hidden(false);
                set_constraint_constant(ah, 40.0);
                if let Some(ref path) = msg.audio_path {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("Audio");
                    self.audio_label.set_text(&format!("🎵 {}", filename));
                }
            }
        } else if let Some(ah) = &self.audio_height {
            self.audio_label.set_hidden(true);
            set_constraint_constant(ah, 0.0);
        }
        // Handle file attachments - show file name
        if msg.file_path.is_some() {
            if let Some(fh) = &self.file_height {
                self.file_label.set_hidden(false);
                set_constraint_constant(fh, 40.0);
                if let Some(ref path) = msg.file_path {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("File");
                    self.file_label.set_text(&format!("📎 {}", filename));
                }
            }
        } else if let Some(fh) = &self.file_height {
            self.file_label.set_hidden(true);
            set_constraint_constant(fh, 0.0);
        }
        // Handle audio attachments - show a play button label
        if msg.audio_path.is_some() {
            if let Some(ah) = &self.audio_height {
                self.audio_label.set_hidden(false);
                set_constraint_constant(ah, 40.0);
                if let Some(ref path) = msg.audio_path {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("Audio");
                    self.audio_label.set_text(&format!("🎵 {}", filename));
                }
            }
        } else if let Some(ah) = &self.audio_height {
            self.audio_label.set_hidden(true);
            set_constraint_constant(ah, 0.0);
        }
        // Handle file attachments - show file name
        if msg.file_path.is_some() {
            if let Some(fh) = &self.file_height {
                self.file_label.set_hidden(false);
                set_constraint_constant(fh, 40.0);
                if let Some(ref path) = msg.file_path {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("File");
                    self.file_label.set_text(&format!("📎 {}", filename));
                }
            }
        } else if let Some(fh) = &self.file_height {
            self.file_label.set_hidden(true);
            set_constraint_constant(fh, 0.0);
        }
        if let Some(ref min_h) = self.bubble_min_height {
            // Ensure the bubble (and thus row) grows for any attachment
            let target = if msg.image_path.is_some() {
                240.0
            } else if msg.video_path.is_some() {
                80.0
            } else if msg.audio_path.is_some() || msg.file_path.is_some() {
                50.0
            } else {
                0.0
            };
            set_constraint_constant(min_h, target);
        }
        if let Some(ref min_w) = self.bubble_min_width {
            let target = if msg.image_path.is_some() {
                380.0
            } else if msg.video_path.is_some() {
                200.0
            } else {
                0.0
            };
            set_constraint_constant(min_w, target);
        }
        if reactions.is_empty() {
            self.reactions_label.set_text("");
            self.reactions_label.set_hidden(true);
        } else {
            self.reactions_label.set_text(&reactions.join("  "));
            self.reactions_label.set_hidden(false);
        }
    }
}

impl ViewDelegate for OutgoingRow {
    const NAME: &'static str = "OutgoingRow";

    fn did_load(&mut self, view: View) {
        self.body_label.set_text_color(Color::Label);
        self.body_label
            .set_line_break_mode(LineBreakMode::WrapWords);
        apply_label_selectable(&self.body_label);
        self.delivery_label.set_font(&Font::system(10.));
        // White text on blue bubble background
        self.delivery_label.set_text_color(Color::SystemWhite);
        self.delivery_label.set_text_alignment(TextAlign::Right);
        self.reactions_label
            .set_font(&cacao::text::Font::system(14.));
        self.reactions_label.set_hidden(true);
        set_react_button_icon(&self.react_button);
        self.react_button.set_action(|sender| {
            let ptr = sender as *const _ as *mut objc::runtime::Object;
            show_emoji_picker_for_row(ptr);
        });
        self.react_button.set_hidden(true);
        set_image_scaling(&self.image_view);
        self.image_view.set_hidden(true);
        self.video_view.set_hidden(true);
        self.audio_label.set_hidden(true);
        self.audio_label.set_font(&Font::system(12.));
        self.audio_label.set_text_color(Color::Label);
        self.file_label.set_hidden(true);
        self.file_label.set_font(&Font::system(12.));
        self.file_label.set_text_color(Color::Label);
        self.bubble.add_subview(&self.body_label);
        self.bubble.add_subview(&self.image_view);
        self.bubble.add_subview(&self.video_view);
        self.bubble.add_subview(&self.audio_label);
        self.bubble.add_subview(&self.file_label);
        self.bubble.add_subview(&self.delivery_label);
        view.add_subview(&self.bubble);
        set_bubble_style(&self.bubble, true);
        view.add_subview(&self.reactions_label);
        view.add_subview(&self.react_button);
        install_hover_tracking(&view, &self.react_button);
        let img_height = self.image_view.height.constraint_equal_to_constant(0.);
        let img_width = self.image_view.width.constraint_equal_to_constant(0.);
        let video_height = self.video_view.height.constraint_equal_to_constant(0.);
        let audio_height = self.audio_label.height.constraint_equal_to_constant(0.);
        let file_height = self.file_label.height.constraint_equal_to_constant(0.);
        let bubble_min_height = self
            .bubble
            .height
            .constraint_greater_than_or_equal_to_constant(0.);
        let bubble_min_width = self
            .bubble
            .width
            .constraint_greater_than_or_equal_to_constant(0.);
        LayoutConstraint::activate(&[
            self.bubble.top.constraint_equal_to(&view.top).offset(4.),
            self.bubble
                .leading
                .constraint_greater_than_or_equal_to(&view.leading)
                .offset(60.),
            self.bubble
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            bubble_min_height.clone(),
            bubble_min_width.clone(),
            self.body_label
                .top
                .constraint_equal_to(&self.bubble.top)
                .offset(8.),
            self.body_label
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(10.),
            self.body_label
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-10.),
            // image view below body label (size starts at 0×0, set dynamically)
            self.image_view
                .top
                .constraint_equal_to(&self.body_label.bottom)
                .offset(4.),
            self.image_view
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(8.),
            img_height.clone(),
            img_width.clone(),
            // video view below image
            self.video_view
                .top
                .constraint_equal_to(&self.image_view.bottom)
                .offset(4.),
            self.video_view
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(8.),
            self.video_view
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-8.),
            video_height.clone(),
            // audio label below video
            self.audio_label
                .top
                .constraint_equal_to(&self.video_view.bottom)
                .offset(4.),
            self.audio_label
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(10.),
            audio_height.clone(),
            // file label below audio
            self.file_label
                .top
                .constraint_equal_to(&self.audio_label.bottom)
                .offset(4.),
            self.file_label
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(10.),
            file_height.clone(),
            // delivery indicator at bottom-right of bubble, below attachments
            self.delivery_label
                .top
                .constraint_equal_to(&self.file_label.bottom)
                .offset(2.),
            self.delivery_label
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-8.),
            self.delivery_label
                .bottom
                .constraint_equal_to(&self.bubble.bottom)
                .offset(-4.),
            self.delivery_label.width.constraint_equal_to_constant(72.),
            // reactions label sits below bubble
            self.reactions_label
                .top
                .constraint_equal_to(&self.bubble.bottom)
                .offset(2.),
            self.reactions_label
                .trailing
                .constraint_equal_to(&self.bubble.trailing),
            self.reactions_label
                .bottom
                .constraint_equal_to(&view.bottom)
                .offset(-4.),
            self.react_button
                .trailing
                .constraint_equal_to(&self.bubble.leading)
                .offset(-4.),
            self.react_button
                .center_y
                .constraint_equal_to(&self.bubble.center_y),
            self.react_button.width.constraint_equal_to_constant(24.),
            self.react_button.height.constraint_equal_to_constant(24.),
        ]);
        self.image_height = Some(img_height);
        self.image_width = Some(img_width);
        self.video_height = Some(video_height);
        self.audio_height = Some(audio_height);
        self.file_height = Some(file_height);
        self.bubble_min_height = Some(bubble_min_height);
        self.bubble_min_width = Some(bubble_min_width);
    }
}

// Incoming message row (gray bubble, left-aligned)

#[derive(Debug)]
pub struct IncomingRow {
    pub bubble: View,
    pub sender_label: Label,
    pub body_label: Label,
    pub image_view: ImageView,
    pub video_view: View,
    pub audio_label: Label,
    pub file_label: Label,
    pub time_label: Label,
    pub reactions_label: Label,
    pub react_button: Button,
    image_height: Option<LayoutConstraint>,
    video_height: Option<LayoutConstraint>,
    audio_height: Option<LayoutConstraint>,
    file_height: Option<LayoutConstraint>,
    bubble_min_height: Option<LayoutConstraint>,
    bubble_min_width: Option<LayoutConstraint>,
}

impl Default for IncomingRow {
    fn default() -> Self {
        Self {
            bubble: View::new(),
            sender_label: Label::new(),
            body_label: Label::new(),
            image_view: ImageView::new(),
            video_view: View::new(),
            audio_label: Label::new(),
            file_label: Label::new(),
            time_label: Label::new(),
            reactions_label: Label::new(),
            react_button: Button::new(""),
            image_height: None,
            video_height: None,
            audio_height: None,
            file_height: None,
            bubble_min_height: None,
            bubble_min_width: None,
        }
    }
}

impl IncomingRow {
    pub fn configure_with(&mut self, msg: &CoreMessage, reactions: &[String]) {
        self.sender_label.set_text(&msg.sender_name);
        let body_text = if let Some(ref q) = msg.quote {
            let quoted = q.text.as_deref().unwrap_or("[attachment]");
            let truncated: String = quoted.chars().take(60).collect();
            let ellipsis = if quoted.chars().count() > 60 {
                "…"
            } else {
                ""
            };
            format!(
                "↩ {}{}\n{}",
                truncated,
                ellipsis,
                msg.body.as_deref().unwrap_or("")
            )
        } else {
            msg.body.as_deref().unwrap_or("").to_string()
        };
        apply_body_text(&self.body_label, &body_text, false);
        self.time_label.set_text(&format_time(msg.timestamp));
        self.react_button.set_hidden(true);
        if let Some(ref c) = self.image_height {
            if let Some(ref path) = msg.image_path {
                let image = Image::with_contents_of_file(&path.to_string_lossy());
                // Always set the image and show the view; NSImage will handle decode failures.
                self.image_view.set_image(&image);
                // Match outgoing thumbnail height for consistency.
                set_constraint_constant(c, 220.0);
                self.image_view.set_hidden(false);
            } else {
                set_constraint_constant(c, 0.0);
                self.image_view.set_hidden(true);
            }
        }
        // Handle video attachments
        if msg.video_path.is_some() {
            if let Some(vh) = &self.video_height {
                self.video_view.set_hidden(false);
                set_constraint_constant(vh, 60.0);
                self.video_view.set_background_color(Color::SystemGray);
                self.video_view.layer.set_corner_radius(8.0);
            }
        } else if let Some(vh) = &self.video_height {
            self.video_view.set_hidden(true);
            set_constraint_constant(vh, 0.0);
        }
        // Handle audio attachments
        if msg.audio_path.is_some() {
            if let Some(ah) = &self.audio_height {
                self.audio_label.set_hidden(false);
                set_constraint_constant(ah, 40.0);
                if let Some(ref path) = msg.audio_path {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("Audio");
                    self.audio_label.set_text(&format!("🎵 {}", filename));
                }
            }
        } else if let Some(ah) = &self.audio_height {
            self.audio_label.set_hidden(true);
            set_constraint_constant(ah, 0.0);
        }
        // Handle file attachments
        if msg.file_path.is_some() {
            if let Some(fh) = &self.file_height {
                self.file_label.set_hidden(false);
                set_constraint_constant(fh, 40.0);
                if let Some(ref path) = msg.file_path {
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("File");
                    self.file_label.set_text(&format!("📎 {}", filename));
                }
            }
        } else if let Some(fh) = &self.file_height {
            self.file_label.set_hidden(true);
            set_constraint_constant(fh, 0.0);
        }
        if let Some(ref min_h) = self.bubble_min_height {
            let target = if msg.image_path.is_some() {
                240.0
            } else if msg.video_path.is_some() {
                80.0
            } else if msg.audio_path.is_some() || msg.file_path.is_some() {
                50.0
            } else {
                0.0
            };
            set_constraint_constant(min_h, target);
        }
        if let Some(ref min_w) = self.bubble_min_width {
            let target = if msg.image_path.is_some() {
                380.0
            } else if msg.video_path.is_some() {
                200.0
            } else {
                0.0
            };
            set_constraint_constant(min_w, target);
        }
        if reactions.is_empty() {
            self.reactions_label.set_text("");
            self.reactions_label.set_hidden(true);
        } else {
            self.reactions_label.set_text(&reactions.join("  "));
            self.reactions_label.set_hidden(false);
        }
    }
}

impl ViewDelegate for IncomingRow {
    const NAME: &'static str = "IncomingRow";

    fn did_load(&mut self, view: View) {
        self.sender_label.set_font(&Font::bold_system(11.));
        self.sender_label.set_text_color(Color::LabelSecondary);
        self.body_label.set_text_color(Color::Label);
        self.body_label
            .set_line_break_mode(LineBreakMode::WrapWords);
        apply_label_selectable(&self.body_label);
        self.reactions_label
            .set_font(&cacao::text::Font::system(14.));
        self.reactions_label.set_hidden(true);
        set_react_button_icon(&self.react_button);
        self.react_button.set_action(|sender| {
            let ptr = sender as *const _ as *mut objc::runtime::Object;
            show_emoji_picker_for_row(ptr);
        });
        self.time_label.set_font(&Font::system(10.));
        self.time_label.set_text_color(Color::LabelSecondary);
        self.time_label.set_text_alignment(TextAlign::Right);
        self.react_button.set_hidden(true);
        set_image_scaling(&self.image_view);
        self.image_view.set_hidden(true);
        self.video_view.set_hidden(true);
        self.audio_label.set_hidden(true);
        self.audio_label.set_font(&Font::system(12.));
        self.audio_label.set_text_color(Color::Label);
        self.file_label.set_hidden(true);
        self.file_label.set_font(&Font::system(12.));
        self.file_label.set_text_color(Color::Label);
        self.bubble.add_subview(&self.body_label);
        self.bubble.add_subview(&self.image_view);
        self.bubble.add_subview(&self.video_view);
        self.bubble.add_subview(&self.audio_label);
        self.bubble.add_subview(&self.file_label);
        self.bubble.add_subview(&self.time_label);
        view.add_subview(&self.sender_label);
        view.add_subview(&self.bubble);
        set_bubble_style(&self.bubble, false);
        view.add_subview(&self.reactions_label);
        view.add_subview(&self.react_button);
        install_hover_tracking(&view, &self.react_button);
        let img_height = self.image_view.height.constraint_equal_to_constant(0.);
        let video_height = self.video_view.height.constraint_equal_to_constant(0.);
        let audio_height = self.audio_label.height.constraint_equal_to_constant(0.);
        let file_height = self.file_label.height.constraint_equal_to_constant(0.);
        let bubble_min_height = self
            .bubble
            .height
            .constraint_greater_than_or_equal_to_constant(0.);
        let bubble_min_width = self
            .bubble
            .width
            .constraint_greater_than_or_equal_to_constant(0.);
        LayoutConstraint::activate(&[
            // sender name above the bubble
            self.sender_label
                .top
                .constraint_equal_to(&view.top)
                .offset(2.),
            self.sender_label
                .leading
                .constraint_equal_to(&view.leading)
                .offset(12.),
            self.sender_label
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-60.),
            self.sender_label.height.constraint_equal_to_constant(16.),
            // bubble below sender name
            self.bubble
                .top
                .constraint_equal_to(&self.sender_label.bottom)
                .offset(2.),
            self.bubble
                .leading
                .constraint_equal_to(&view.leading)
                .offset(12.),
            self.bubble
                .trailing
                .constraint_less_than_or_equal_to(&view.trailing)
                .offset(-60.),
            bubble_min_height.clone(),
            bubble_min_width.clone(),
            self.body_label
                .top
                .constraint_equal_to(&self.bubble.top)
                .offset(8.),
            self.body_label
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(10.),
            self.body_label
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-10.),
            // image view below body label (height starts at 0, set dynamically)
            self.image_view
                .top
                .constraint_equal_to(&self.body_label.bottom)
                .offset(4.),
            self.image_view
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(8.),
            self.image_view
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-8.),
            img_height.clone(),
            // video view below image
            self.video_view
                .top
                .constraint_equal_to(&self.image_view.bottom)
                .offset(4.),
            self.video_view
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(8.),
            self.video_view
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-8.),
            video_height.clone(),
            // audio label below video
            self.audio_label
                .top
                .constraint_equal_to(&self.video_view.bottom)
                .offset(4.),
            self.audio_label
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(10.),
            audio_height.clone(),
            // file label below audio
            self.file_label
                .top
                .constraint_equal_to(&self.audio_label.bottom)
                .offset(4.),
            self.file_label
                .leading
                .constraint_equal_to(&self.bubble.leading)
                .offset(10.),
            file_height.clone(),
            // time label at bottom-right of bubble, below attachments
            self.time_label
                .top
                .constraint_equal_to(&self.file_label.bottom)
                .offset(2.),
            self.time_label
                .trailing
                .constraint_equal_to(&self.bubble.trailing)
                .offset(-8.),
            self.time_label
                .bottom
                .constraint_equal_to(&self.bubble.bottom)
                .offset(-4.),
            self.time_label.width.constraint_equal_to_constant(40.),
            self.reactions_label
                .top
                .constraint_equal_to(&self.bubble.bottom)
                .offset(2.),
            self.reactions_label
                .leading
                .constraint_equal_to(&self.bubble.leading),
            self.reactions_label
                .bottom
                .constraint_equal_to(&view.bottom)
                .offset(-4.),
            self.react_button
                .leading
                .constraint_equal_to(&self.bubble.trailing)
                .offset(4.),
            self.react_button
                .center_y
                .constraint_equal_to(&self.bubble.center_y),
            self.react_button.width.constraint_equal_to_constant(24.),
            self.react_button.height.constraint_equal_to_constant(24.),
        ]);
        self.image_height = Some(img_height);
        self.video_height = Some(video_height);
        self.audio_height = Some(audio_height);
        self.file_height = Some(file_height);
        self.bubble_min_height = Some(bubble_min_height);
        self.bubble_min_width = Some(bubble_min_width);
    }
}

// Call message row

const CALL_ROW: &str = "CallCell";

#[derive(Default, Debug)]
pub struct CallRow {
    pub container: View,
    pub icon: Label,
    pub text: Label,
}

impl CallRow {
    pub fn configure_with(&mut self, msg: &CoreMessage) {
        let call_text = msg.body.clone().unwrap_or_else(|| "Call".to_string());
        self.text.set_text(&call_text);
    }
}

impl ViewDelegate for CallRow {
    const NAME: &'static str = "CallRow";

    fn did_load(&mut self, view: View) {
        use cacao::text::Font;
        self.container.set_background_color(Color::SystemGray5);

        self.icon.set_text("📞");
        self.icon.set_font(&Font::system(14.));
        self.text.set_text_color(Color::SystemGray);
        self.text.set_font(&Font::system(12.));

        self.container.add_subview(&self.icon);
        self.container.add_subview(&self.text);
        view.add_subview(&self.container);

        LayoutConstraint::activate(&[
            self.container.center_x.constraint_equal_to(&view.center_x),
            self.container.top.constraint_equal_to(&view.top).offset(4.),
            self.container
                .bottom
                .constraint_equal_to(&view.bottom)
                .offset(-4.),
            self.container
                .height
                .constraint_greater_than_or_equal_to_constant(28.),
            self.icon
                .leading
                .constraint_equal_to(&self.container.leading)
                .offset(12.),
            self.icon
                .center_y
                .constraint_equal_to(&self.container.center_y),
            self.text
                .leading
                .constraint_equal_to(&self.icon.trailing)
                .offset(6.),
            self.text
                .trailing
                .constraint_equal_to(&self.container.trailing)
                .offset(-12.),
            self.text
                .center_y
                .constraint_equal_to(&self.container.center_y),
        ]);
    }
}

// Date divider row

#[derive(Default, Debug)]
pub struct DateDividerRow {
    pub label: Label,
}

impl DateDividerRow {
    pub fn configure_with(&mut self, text: &str) {
        self.label.set_text(text);
    }
}

impl ViewDelegate for DateDividerRow {
    const NAME: &'static str = "DateDividerRow";

    fn did_load(&mut self, view: View) {
        self.label.set_text_color(Color::SystemGray);
        self.label.set_text_alignment(TextAlign::Center);
        view.add_subview(&self.label);
        LayoutConstraint::activate(&[
            self.label.top.constraint_equal_to(&view.top).offset(8.),
            self.label
                .bottom
                .constraint_equal_to(&view.bottom)
                .offset(-8.),
            self.label
                .leading
                .constraint_equal_to(&view.leading)
                .offset(12.),
            self.label
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
        ]);
    }
}

// Reaction row

// List delegate

#[derive(Debug)]
pub struct MessageListDelegate {
    pub view: Option<ListView>,
    items: RefCell<Vec<MessageItem>>,
    selected_row: RefCell<Option<usize>>,
}

impl Default for MessageListDelegate {
    fn default() -> Self {
        Self {
            view: None,
            items: RefCell::new(Vec::new()),
            selected_row: RefCell::new(None),
        }
    }
}

impl MessageListDelegate {
    pub fn set_messages(&self, messages: Vec<CoreMessage>) {
        // Reverse so oldest is at top, newest at bottom (like normal chat)
        let messages: Vec<_> = messages.into_iter().rev().collect();
        let items = build_message_items(messages);
        rebuild_row_attachments(&items);
        *self.items.borrow_mut() = items;
        if let Some(view) = &self.view {
            reload_listview(view);
            // Scroll to last row (newest message)
            let ptr = view
                .objc
                .get(|obj| obj as *const _ as *mut objc::runtime::Object);
            unsafe {
                use objc::{msg_send, sel, sel_impl};
                let count: usize = msg_send![ptr, numberOfRows];
                if count > 0 {
                    // Force layout first, then scroll
                    let _: () = msg_send![ptr, layoutSubtreeIfNeeded];
                    let _: () = msg_send![ptr, scrollRowToVisible: (count - 1) as isize];
                }
            }
        }
    }

    pub fn add_message(&self, message: CoreMessage) {
        let is_reaction = message.is_reaction;
        let is_outgoing = message.is_outgoing;
        let reaction_target_ts = message.reaction_target_ts;

        {
            let mut items = self.items.borrow_mut();

            if message.is_reaction {
                // Reactions update an existing message — no new row, no date divider
                if let Some(ts) = reaction_target_ts {
                    let sender = message.sender_name.clone();
                    let emoji = message.body.as_deref().unwrap_or("").to_string();
                    for (idx, item) in items.iter_mut().enumerate().rev() {
                        match item {
                            MessageItem::Outgoing(m, reactions)
                            | MessageItem::Incoming(m, reactions)
                                if m.timestamp == ts =>
                            {
                                reactions.retain(|r| {
                                    r.split_once(' ').map_or(true, |(_, s)| s != &sender)
                                });
                                if !message.reaction_remove {
                                    reactions.push(format!("{} {}", emoji, sender));
                                }
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                let need_divider = items.last().map_or(true, |last| {
                    let last_ts = last.timestamp();
                    if last_ts == 0 {
                        return false;
                    }
                    let last_date = chrono::DateTime::from_timestamp((last_ts / 1000) as i64, 0)
                        .map(|dt| dt.date_naive());
                    let new_date =
                        chrono::DateTime::from_timestamp((message.timestamp / 1000) as i64, 0)
                            .map(|dt| dt.date_naive());
                    last_date != new_date
                });
                if need_divider {
                    items.push(MessageItem::DateDivider(date_label_for_timestamp(
                        message.timestamp,
                    )));
                }
                if message.is_outgoing {
                    items.push(MessageItem::Outgoing(message, vec![]));
                } else {
                    items.push(MessageItem::Incoming(message, vec![]));
                }
            }

            rebuild_row_attachments(&items);
        }
        if let Some(view) = &self.view {
            if !is_reaction {
                reload_listview(view);
                scroll_to_bottom(view, is_outgoing);
            } else {
                // For reactions: reload just the affected row to avoid scroll jump
                if let Some(ts) = reaction_target_ts {
                    let items = self.items.borrow();
                    if let Some(idx) = items.iter().position(|item| match item {
                        MessageItem::Outgoing(m, _)
                        | MessageItem::Incoming(m, _)
                        | MessageItem::Call(m) => m.timestamp == ts,
                        MessageItem::DateDivider(_) => false,
                    }) {
                        let ptr = view
                            .objc
                            .get(|obj| obj as *const _ as *mut objc::runtime::Object);
                        unsafe {
                            use objc::{class, msg_send, sel, sel_impl};
                            let index_set: *mut objc::runtime::Object =
                                msg_send![class!(NSIndexSet), indexSetWithIndex: idx];
                            let col_set: *mut objc::runtime::Object =
                                msg_send![class!(NSIndexSet), indexSetWithIndex: 0];
                            let _: () = msg_send![ptr, reloadDataForRowIndexes: index_set columnIndexes: col_set];
                        }
                    }
                }
            }
        }
    }

    pub fn prepend_messages(&self, older: Vec<CoreMessage>) {
        let mut items = self.items.borrow_mut();
        let mut new_items = build_message_items(older);
        new_items.append(&mut *items);
        *items = new_items;
        rebuild_row_attachments(&items);
        drop(items);
        if let Some(view) = &self.view {
            reload_listview(view);
        }
    }

    /// Return the effective row index: prefer clickedRow, then cached context row, then selection.
    fn effective_row(&self) -> Option<usize> {
        if let Some(ref view) = self.view {
            let ptr = view
                .objc
                .get(|obj| obj as *const _ as *mut objc::runtime::Object);
            let clicked: isize = unsafe {
                use objc::{msg_send, sel, sel_impl};
                msg_send![ptr, clickedRow]
            };
            if clicked >= 0 {
                return Some(clicked as usize);
            }
        }
        let cached = LAST_CLICKED_ROW.load(Ordering::Relaxed);
        if cached >= 0 {
            return Some(cached as usize);
        }
        *self.selected_row.borrow()
    }

    pub fn selected_message_body(&self) -> Option<String> {
        let idx = self.effective_row()?;
        let items = self.items.borrow();
        match items.get(idx) {
            Some(
                MessageItem::Outgoing(m, _) | MessageItem::Incoming(m, _) | MessageItem::Call(m),
            ) => m.body.clone(),
            _ => None,
        }
    }

    pub fn selected_message(&self) -> Option<CoreMessage> {
        let idx = self.effective_row()?;
        let items = self.items.borrow();
        match items.get(idx) {
            Some(
                MessageItem::Outgoing(m, _) | MessageItem::Incoming(m, _) | MessageItem::Call(m),
            ) => Some(m.clone()),
            _ => None,
        }
    }

    pub fn remove_message(&self, timestamp: u64) {
        {
            let mut items = self.items.borrow_mut();
            items.retain(|item| match item {
                MessageItem::Outgoing(m, _)
                | MessageItem::Incoming(m, _)
                | MessageItem::Call(m) => m.timestamp != timestamp,
                MessageItem::DateDivider(_) => true,
            });
            rebuild_row_attachments(&items);
        }
        if let Some(ref view) = self.view {
            reload_listview(view);
        }
    }
}

impl ListViewDelegate for MessageListDelegate {
    const NAME: &'static str = "MessageListView";

    fn did_load(&mut self, view: ListView) {
        view.register(MSG_OUT_ROW, OutgoingRow::default);
        view.register(MSG_IN_ROW, IncomingRow::default);
        view.register(CALL_ROW, CallRow::default);
        view.register(DATE_ROW, DateDividerRow::default);
        // Let NSTableView determine row heights from the view layout so
        // image bubbles can grow to their full size.
        view.set_uses_automatic_row_heights(true);

        // Cacao's ScrollView sets setDrawsBackground:NO by default, making the
        // list transparent. Re-enable it so the message area has an opaque background.
        view.scrollview.objc.with_mut(|obj| unsafe {
            use objc::{msg_send, sel, sel_impl};
            let _: () = msg_send![obj, setDrawsBackground: objc::runtime::YES];
        });

        let view_ptr = view
            .objc
            .get(|obj| obj as *const _ as *mut objc::runtime::Object);

        // Store table view pointer BEFORE attaching context menu (needed for double-click handler)
        TABLE_VIEW_PTR.store(view_ptr as usize, Ordering::Relaxed);

        attach_message_context_menu(
            view_ptr,
            || App::<FlareApp, AppMessage>::dispatch_main(AppMessage::CopySelectedMessage),
            || App::<FlareApp, AppMessage>::dispatch_main(AppMessage::ReplyToSelectedMessage),
            || App::<FlareApp, AppMessage>::dispatch_main(AppMessage::DeleteSelectedMessage),
        );

        setup_table_double_action(view_ptr);

        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.items.borrow().len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        // Wrap in catch_unwind to prevent panics from aborting inside extern "C".
        // This lets us log the actual panic message for debugging.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let items = self.items.borrow();
            match items.get(row) {
                Some(MessageItem::Outgoing(msg, reactions)) => {
                    let mut view = self
                        .view
                        .as_ref()
                        .unwrap()
                        .dequeue::<OutgoingRow>(MSG_OUT_ROW);
                    if let Some(delegate) = &mut view.delegate {
                        delegate.configure_with(msg, reactions);
                    }
                    view.into_row()
                }
                Some(MessageItem::Incoming(msg, reactions)) => {
                    let mut view = self
                        .view
                        .as_ref()
                        .unwrap()
                        .dequeue::<IncomingRow>(MSG_IN_ROW);
                    if let Some(delegate) = &mut view.delegate {
                        delegate.configure_with(msg, reactions);
                    }
                    view.into_row()
                }
                Some(MessageItem::Call(msg)) => {
                    let mut view = self.view.as_ref().unwrap().dequeue::<CallRow>(CALL_ROW);
                    if let Some(delegate) = &mut view.delegate {
                        delegate.configure_with(msg);
                    }
                    view.into_row()
                }
                Some(MessageItem::DateDivider(text)) => {
                    let mut view = self
                        .view
                        .as_ref()
                        .unwrap()
                        .dequeue::<DateDividerRow>(DATE_ROW);
                    if let Some(delegate) = &mut view.delegate {
                        delegate.configure_with(text);
                    }
                    view.into_row()
                }
                None => {
                    let view = self
                        .view
                        .as_ref()
                        .unwrap()
                        .dequeue::<OutgoingRow>(MSG_OUT_ROW);
                    view.into_row()
                }
            }
        }));
        match result {
            Ok(row) => row,
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };
                log::error!("PANIC in item_for({}): {}", row, msg);
                // Return a fallback empty row
                let view = self
                    .view
                    .as_ref()
                    .unwrap()
                    .dequeue::<OutgoingRow>(MSG_OUT_ROW);
                view.into_row()
            }
        }
    }

    fn item_selected(&self, row: Option<usize>) {
        *self.selected_row.borrow_mut() = row;
    }
}
