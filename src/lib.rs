#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

//! Minecraft Java Edition 1.12.2 client port.
//! Source paths mirror the MCP 1.12.2 package/class layout. Native Vulkan and
//! OpenGL implementation details remain under `src/vulkan` and `src/opengl`;
//! launcher, backend selection and Java compatibility support stay outside the
//! MCP package tree.

pub mod com;
pub mod compat;
pub mod launcher;
pub mod net;
#[cfg(not(target_os = "android"))]
pub mod opengl;
pub mod renderer;
pub mod vulkan;

/// Android NativeActivity entry point, exported by the cdylib.
///
/// The desktop `main()` (src/main.rs) drives Windows builds; Android loads
/// this shared library and calls `android_main` once the app is resumed.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: winit::platform::android::activity::AndroidApp) {
    launcher::android::set_android_app(app.clone());
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("mc112"),
    );
    let game_dir = launcher::android::game_dir();
    if let Err(error) = launcher::AssetBootstrap::extract_assets(&app.asset_manager(), &game_dir) {
        log::error!("asset extraction failed: {error:#}");
        return;
    }
    let assets = game_dir.join("assets");
    log::info!("Minecraft 1.12.2 Android entry: gameDir={}", game_dir.display());
    let args: Vec<String> = vec![
        "mc112-client".to_owned(),
        "run".to_owned(),
        "--assets".to_owned(),
        assets.to_string_lossy().into_owned(),
    ];
    let _ = net::minecraft::client::main::Main::main(args);
}

pub const GAME_VERSION: &str = "1.12.2";
pub const PROTOCOL_VERSION: i32 = 340;
