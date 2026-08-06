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
}

#[cfg(target_os = "android")]
pub use platform::*;
