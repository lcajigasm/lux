//! Android JNI surface. Desktop builds only expose [`available`].

pub fn available() -> bool {
    cfg!(target_os = "android")
}

#[cfg(target_os = "android")]
mod jni {
    use std::ffi::{CStr, c_char, c_void};
    use std::sync::Mutex;

    static PENDING_OPEN_URIS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static PENDING_INPUT: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[unsafe(no_mangle)]
    pub extern "C" fn Java_lux_android_host_LuxActivity_nativeOpenUri(
        _env: *mut c_void,
        _this: *mut c_void,
        uri: *const c_char,
    ) {
        if uri.is_null() {
            return;
        }
        if let Ok(s) = unsafe { CStr::from_ptr(uri) }.to_str() {
            if let Ok(mut uris) = PENDING_OPEN_URIS.lock() {
                uris.push(s.to_string());
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn Java_lux_android_host_LuxActivity_nativeInsertText(
        _env: *mut c_void,
        _this: *mut c_void,
        text: *const c_char,
    ) {
        if text.is_null() {
            return;
        }
        if let Ok(s) = unsafe { CStr::from_ptr(text) }.to_str() {
            if let Ok(mut pending) = PENDING_INPUT.lock() {
                pending.push(s.to_string());
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn Java_lux_android_host_LuxActivity_nativeDeleteBackward(
        _env: *mut c_void,
        _this: *mut c_void,
    ) {
        if let Ok(mut pending) = PENDING_INPUT.lock() {
            pending.push("\x08".to_string());
        }
    }

    pub fn drain_open_uris() -> Vec<String> {
        if let Ok(mut uris) = PENDING_OPEN_URIS.lock() {
            std::mem::take(&mut *uris)
        } else {
            Vec::new()
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
    fn kotlin_host_uses_input_connection_and_saf() {
        let src = include_str!("../../../host-android/LuxActivity.kt");
        assert!(src.contains("InputConnection"));
        assert!(src.contains("SurfaceView"));
        assert!(src.contains("ACTION_OPEN_DOCUMENT"));
        assert!(src.contains("onCreateInputConnection"));
        assert!(src.contains("loadLibrary"));
        assert_eq!(super::available(), cfg!(target_os = "android"));
    }
}
