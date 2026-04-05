// Prevents additional console window on Windows in release mode.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK can crash on Wayland compositors without this hint.
    #[cfg(target_os = "linux")]
    // SAFETY: called before any threads are spawned, at the top of main.
    unsafe {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1")
    };

    gif_editor_lib::run();
}
