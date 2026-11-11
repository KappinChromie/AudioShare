pub mod audio;
pub mod commands;
pub mod enumerator;

use std::sync::Mutex;
use audio::AudioEngine;
use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        .manage(AppState(Mutex::new(AudioEngine::new())))
        .invoke_handler(tauri::generate_handler![
            commands::get_devices,
            commands::get_running_apps,
            commands::start_streaming,
            commands::stop_streaming,
            commands::set_app_volume,
            commands::set_mic_volume,
            commands::set_app_normalize,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
