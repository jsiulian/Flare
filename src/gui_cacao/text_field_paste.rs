use cacao::appkit::App;
use cacao::input::TextField;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

use super::app::{AppMessage, FlareApp};

pub fn setup_paste_handler(_text_field: &TextField) {
    // Paste handler is not implemented yet - attachments can be added via the attach button
}

pub fn remove_paste_handler() {}

fn check_clipboard_and_handle() {
    unsafe {
        let pasteboard: *mut objc::runtime::Object =
            msg_send![class!(NSPasteboard), generalPasteboard];

        let file_url_type = nsstring("public.file-url");
        let file_url: *mut objc::runtime::Object =
            msg_send![pasteboard, stringForType: file_url_type];

        if !file_url.is_null() {
            let url_string: *mut objc::runtime::Object = msg_send![file_url, absoluteString];
            if !url_string.is_null() {
                let ns_url: *mut objc::runtime::Object =
                    msg_send![class!(NSURL), URLWithString: url_string];
                if !ns_url.is_null() {
                    let path: *mut objc::runtime::Object = msg_send![ns_url, path];
                    if !path.is_null() {
                        let path_string: *mut objc::runtime::Object = msg_send![path, UTF8String];
                        if !path_string.is_null() {
                            let c_string: *const std::os::raw::c_char =
                                msg_send![path_string, UTF8String];
                            let bytes = std::slice::from_raw_parts(c_string as *const u8, 1000);
                            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                            let path_str = String::from_utf8_lossy(&bytes[..end]).to_string();
                            App::<FlareApp, AppMessage>::dispatch_main(AppMessage::PasteFile(
                                path_str,
                            ));
                            return;
                        }
                    }
                }
            }
        }

        let png_type = nsstring("public.png");
        let png_data: *mut objc::runtime::Object = msg_send![pasteboard, dataForType: png_type];

        if !png_data.is_null() {
            let length: usize = msg_send![png_data, length];
            if length > 0 {
                let bytes: *const std::ffi::c_void = msg_send![png_data, bytes];
                let data = std::slice::from_raw_parts(bytes as *const u8, length).to_vec();
                let filename = format!(
                    "clipboard_{}.png",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                );
                App::<FlareApp, AppMessage>::dispatch_main(AppMessage::PasteImage(data, filename));
                return;
            }
        }

        let tiff_type = nsstring("public.tiff");
        let tiff_data: *mut objc::runtime::Object = msg_send![pasteboard, dataForType: tiff_type];

        if !tiff_data.is_null() {
            let length: usize = msg_send![tiff_data, length];
            if length > 0 {
                let bytes: *const std::ffi::c_void = msg_send![tiff_data, bytes];
                let data = std::slice::from_raw_parts(bytes as *const u8, length).to_vec();
                let png_data = convert_tiff_to_png(&data);
                if !png_data.is_empty() {
                    let filename = format!(
                        "clipboard_{}.png",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                    );
                    App::<FlareApp, AppMessage>::dispatch_main(AppMessage::PasteImage(
                        png_data, filename,
                    ));
                    return;
                }
            }
        }
    }
}

unsafe fn nsstring(s: &str) -> *mut objc::runtime::Object {
    let cs = std::ffi::CString::new(s).unwrap_or_default();
    msg_send![class!(NSString), stringWithUTF8String: cs.as_ptr()]
}

fn convert_tiff_to_png(tiff_data: &[u8]) -> Vec<u8> {
    use std::io::Cursor;

    if let Ok(img) = image::load_from_memory(tiff_data) {
        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        if img.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
            return png_data;
        }
    }
    Vec::new()
}
