//! UIKit FFI surface compiled into the iOS staticlib. Desktop builds
//! only expose [`available`].

pub fn available() -> bool {
    cfg!(target_os = "ios")
}

#[cfg(target_os = "ios")]
mod ffi {
    use std::ffi::{CStr, c_char};
    use std::sync::Mutex;

    static PENDING_INPUT: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[unsafe(no_mangle)]
    pub extern "C" fn lux_ios_insert_text(ptr: *const c_char) {
        if ptr.is_null() {
            return;
        }
        if let Ok(s) = unsafe { CStr::from_ptr(ptr) }.to_str() {
            if let Ok(mut pending) = PENDING_INPUT.lock() {
                pending.push(s.to_string());
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn lux_ios_delete_backward() {
        if let Ok(mut pending) = PENDING_INPUT.lock() {
            pending.push("\x08".to_string());
        }
    }

    pub fn drain_input() -> Vec<String> {
        if let Ok(mut pending) = PENDING_INPUT.lock() {
            std::mem::take(&mut *pending)
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn swift_host_owns_metal_and_ime_not_winit() {
        let src = include_str!("../../../host-ios/LuxViewController.swift");
        assert!(src.contains("MTKView"));
        assert!(src.contains("UIKeyInput"));
        assert!(src.contains("UIDocumentPickerViewController"));
        assert!(src.contains("UIKeyCommand"));
        assert!(src.contains("CAMetalLayer") || src.contains("MTKView"));
        assert_eq!(super::available(), cfg!(target_os = "ios"));
    }
}
