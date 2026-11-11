use std::sync::Mutex;
use tauri::State;

use crate::audio::{AudioEngine, StreamConfig};
use crate::enumerator::{get_device_lists, list_audio_applications, AppEntry, DeviceLists};

pub struct AppState(pub Mutex<AudioEngine>);

#[tauri::command]
pub fn get_devices() -> Result<DeviceLists, String> {
    get_device_lists()
}

#[tauri::command]
pub fn get_running_apps() -> Result<Vec<AppEntry>, String> {
    list_audio_applications()
}

#[tauri::command]
pub fn start_streaming(state: State<'_, AppState>, config: StreamConfig) -> Result<(), String> {
    let mut engine = state
        .0
        .lock()
        .map_err(|e| format!("Failed to acquire engine lock: {e}"))?;
    engine.start(config)
}

#[tauri::command]
pub fn stop_streaming(state: State<'_, AppState>) -> Result<(), String> {
    let mut engine = state
        .0
        .lock()
        .map_err(|e| format!("Failed to acquire engine lock: {e}"))?;
    engine.stop()
}

#[tauri::command]
pub fn set_app_volume(state: State<'_, AppState>, volume: f32) -> Result<(), String> {
    let engine = state
        .0
        .lock()
        .map_err(|e| format!("Failed to acquire engine lock: {e}"))?;
    engine.set_app_volume(volume)
}

#[tauri::command]
pub fn set_mic_volume(state: State<'_, AppState>, volume: f32) -> Result<(), String> {
    let engine = state
        .0
        .lock()
        .map_err(|e| format!("Failed to acquire engine lock: {e}"))?;
    engine.set_mic_volume(volume)
}

#[tauri::command]
pub fn set_app_normalize(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let engine = state
        .0
        .lock()
        .map_err(|e| format!("Failed to acquire engine lock: {e}"))?;
    engine.set_app_normalize(enabled)
}
