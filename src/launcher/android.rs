//! Android platform glue: global AndroidApp holder and game-dir resolution.
//! Everything here is compiled out of Windows builds.

#[cfg(target_os = "android")]
mod platform {
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use winit::platform::android::activity::AndroidApp;

    static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();

    pub fn set_android_app(app: AndroidApp) {
        let _ = ANDROID_APP.set(app);
    }

    pub fn android_app() -> &'static AndroidApp {
        ANDROID_APP.get().expect("android_main must set the AndroidApp before use")
    }

    /// The app-internal data directory (getFilesDir equivalent). Used as gameDir.
    pub fn game_dir() -> PathBuf {
        android_app()
            .internal_data_path()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    }

    /// Opens a URL in the system browser via an ACTION_VIEW intent. Android
    /// has no xdg-open; the upstream OAuth flow otherwise fails to launch.
    pub fn open_url(url: &str) -> bool {
        use jni::objects::{JObject, JValue};

        let app = android_app();
        let Ok(vm) = (unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }) else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let Some(url_string) = env.new_string(url).ok() else { return false; };
        let url_obj = JObject::from(url_string);
        let Ok(uri) = env.call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&url_obj)],
        ) else {
            return false;
        };
        let Ok(uri) = uri.l() else { return false; };
        let Ok(action) = env.new_string("android.intent.action.VIEW") else { return false; };
        let action_obj = JObject::from(action);
        let Ok(intent) = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;Landroid/net/Uri;)V",
            &[JValue::Object(&action_obj), JValue::Object(&uri)],
        ) else {
            return false;
        };
        match env.call_method(&activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)]) {
            Ok(_) => true,
            Err(error) => {
                log::error!("failed starting browser intent: {error}");
                false
            }
        }
    }

    /// Physical screen size in pixels (DisplayMetrics). Touch coordinates on
    /// Android are screen pixels, while the swapchain renders at the
    /// ANativeWindow size (which excludes the system bar); hit-testing needs
    /// the ratio between the two.
    pub fn screen_size() -> (u32, u32) {
        use jni::objects::JObject;

        let app = android_app();
        let Ok(vm) = (unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }) else {
            return (0, 0);
        };
        let Ok(mut env) = vm.attach_current_thread() else { return (0, 0); };
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let Ok(resources) = env.call_method(&activity, "getResources", "()Landroid/content/res/Resources;", &[]) else {
            return (0, 0);
        };
        let Ok(resources) = resources.l() else { return (0, 0); };
        let Ok(metrics) = env.call_method(&resources, "getDisplayMetrics", "()Landroid/util/DisplayMetrics;", &[]) else {
            return (0, 0);
        };
        let Ok(metrics) = metrics.l() else { return (0, 0); };
        let width = env.get_field(&metrics, "widthPixels", "I").and_then(|v| v.i()).unwrap_or(0);
        let height = env.get_field(&metrics, "heightPixels", "I").and_then(|v| v.i()).unwrap_or(0);
        (width as u32, height as u32)
    }

    /// Hides the status and navigation bars so the game surface covers the
    /// whole display. Without this the system bar shrinks the window while
    /// the Vulkan surface stays full-size, which makes every present report
    /// VK_SUBOPTIMAL_KHR. Must run before the event loop creates the surface.
    pub fn enter_fullscreen() {
        use winit::platform::android::activity::WindowManagerFlags;
        android_app().set_window_flags(
            WindowManagerFlags::FULLSCREEN,
            WindowManagerFlags::FORCE_NOT_FULLSCREEN,
        );
        log::info!("requested immersive fullscreen");
    }
}

#[cfg(target_os = "android")]
pub use platform::*;
