//! Android JNI surface. Desktop builds only expose [`available`].

pub fn available() -> bool {
    cfg!(target_os = "android")
}

#[cfg(target_os = "android")]
mod jni {
    use std::ffi::{CStr, c_char};

    #[unsafe(no_mangle)]
    pub extern "C" fn Java_lux_android_host_LuxActivity_nativeOpenUri(
        _env: *mut std::ffi::c_void,
        _this: *mut std::ffi::c_void,
        uri: *const c_char,
    ) {
        if uri.is_null() {
            return;
        }
        let _ = unsafe { CStr::from_ptr(uri) }.to_str();
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
