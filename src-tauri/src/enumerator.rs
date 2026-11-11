use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use sysinfo::System;
use wasapi::{DeviceEnumerator, Direction};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEntry {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub pid: u32,
    pub name: String,
    pub exe_name: String,
    pub is_audio_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLists {
    pub microphones: Vec<DeviceEntry>,
    pub virtual_outputs: Vec<DeviceEntry>,
    pub cable_installed: bool,
}

pub fn list_microphones() -> Result<Vec<DeviceEntry>, String> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new().map_err(|e| format!("Failed to create device enumerator: {e}"))?;
    let default_id = enumerator
        .get_default_device(&Direction::Capture)
        .ok()
        .and_then(|d| d.get_id().ok());

    let collection = enumerator
        .get_device_collection(&Direction::Capture)
        .map_err(|e| format!("Failed to enumerate capture devices: {e}"))?;

    let mut list = Vec::new();
    for dev_res in &collection {
        if let Ok(dev) = dev_res {
            let id = dev.get_id().unwrap_or_default();
            let name = dev.get_friendlyname().unwrap_or_else(|_| "Unknown Device".into());
            let is_default = default_id.as_deref() == Some(&id);
            let name_lower = name.to_lowercase();
            let is_virtual = name_lower.contains("cable")
                || name_lower.contains("virtual")
                || name_lower.contains("blackhole")
                || name_lower.contains("stream");

            list.push(DeviceEntry {
                id,
                name,
                is_default,
                is_virtual,
            });
        }
    }

    Ok(list)
}

pub fn list_virtual_outputs() -> Result<Vec<DeviceEntry>, String> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new().map_err(|e| format!("Failed to create device enumerator: {e}"))?;
    let default_id = enumerator
        .get_default_device(&Direction::Render)
        .ok()
        .and_then(|d| d.get_id().ok());

    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| format!("Failed to enumerate render devices: {e}"))?;

    let mut list = Vec::new();
    for dev_res in &collection {
        if let Ok(dev) = dev_res {
            let id = dev.get_id().unwrap_or_default();
            let name = dev.get_friendlyname().unwrap_or_else(|_| "Unknown Render Device".into());
            let is_default = default_id.as_deref() == Some(&id);
            let name_lower = name.to_lowercase();
            let is_virtual = name_lower.contains("cable input")
                || name_lower.contains("cable in")
                || name_lower.contains("blackhole")
                || name_lower.contains("virtual")
                || name_lower.contains("vb-audio");

            list.push(DeviceEntry {
                id,
                name,
                is_default,
                is_virtual,
            });
        }
    }

    // Sort list so virtual / cable devices are at the top
    list.sort_by(|a, b| {
        b.is_virtual
            .cmp(&a.is_virtual)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(list)
}

pub fn get_device_lists() -> Result<DeviceLists, String> {
    let microphones = list_microphones()?;
    let virtual_outputs = list_virtual_outputs()?;
    let cable_installed = virtual_outputs
        .iter()
        .any(|d| d.name.to_lowercase().contains("cable input") || d.name.to_lowercase().contains("cable in"));

    Ok(DeviceLists {
        microphones,
        virtual_outputs,
        cable_installed,
    })
}

fn format_friendly_app_name(raw_name: &str) -> String {
    let lower = raw_name.to_lowercase();
    match lower.as_str() {
        "discord.exe" | "discord" => "Discord".into(),
        "spotify.exe" | "spotify" => "Spotify".into(),
        "chrome.exe" | "chrome" => "Google Chrome".into(),
        "msedge.exe" | "msedge" => "Microsoft Edge".into(),
        "firefox.exe" | "firefox" => "Mozilla Firefox".into(),
        "steam.exe" | "steam" => "Steam".into(),
        "code.exe" | "code" => "Visual Studio Code".into(),
        "devenv.exe" => "Visual Studio".into(),
        "vlc.exe" | "vlc" => "VLC Media Player".into(),
        "obs64.exe" | "obs32.exe" | "obs.exe" => "OBS Studio".into(),
        "telegram.exe" | "telegram" => "Telegram".into(),
        "slack.exe" | "slack" => "Slack".into(),
        "teams.exe" | "ms-teams.exe" => "Microsoft Teams".into(),
        "antigravity.exe" => "Antigravity".into(),
        _ => {
            let without_exe = if lower.ends_with(".exe") {
                &raw_name[..raw_name.len() - 4]
            } else {
                raw_name
            };
            // Capitalize first letter if all lowercase
            if without_exe.chars().all(|c| c.is_lowercase() || !c.is_alphabetic()) {
                let mut chars = without_exe.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            } else {
                without_exe.to_string()
            }
        }
    }
}

pub fn list_audio_applications() -> Result<Vec<AppEntry>, String> {
    let _ = wasapi::initialize_mta();
    let mut active_audio_pids = HashSet::new();

    // Query active WASAPI audio sessions from default render endpoint
    if let Ok(enumerator) = DeviceEnumerator::new() {
        if let Ok(render_dev) = enumerator.get_default_device(&Direction::Render) {
            if let Ok(session_mgr) = render_dev.get_iaudiosessionmanager() {
                if let Ok(session_enum) = session_mgr.get_audiosessionenumerator() {
                    if let Ok(count) = session_enum.get_count() {
                        for i in 0..count {
                            if let Ok(session) = session_enum.get_session(i) {
                                if let Ok(pid) = session.get_process_id() {
                                    if pid > 4 {
                                        active_audio_pids.insert(pid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut sys = System::new_all();
    sys.refresh_all();

    let current_pid = std::process::id();

    // System processes and background daemons to ignore unless they are actively playing audio
    let ignored_names: HashSet<&'static str> = [
        "system",
        "idle",
        "registry",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "lsass.exe",
        "svchost.exe",
        "fontdrvhost.exe",
        "dwm.exe",
        "memory compression",
        "spoolsv.exe",
        "conhost.exe",
        "sihost.exe",
        "taskhostw.exe",
        "explorer.exe",
        "searchhost.exe",
        "startmenuexperiencehost.exe",
        "shellexperiencehost.exe",
        "lockapp.exe",
        "securityhealthsystray.exe",
        "ctfmon.exe",
        "runtimebroker.exe",
        "smartscreen.exe",
        "dllhost.exe",
        "searchindexer.exe",
        "wlanext.exe",
        "winlogon.exe",
    ]
    .into_iter()
    .collect();

    // Deduplicate by application (one entry per application)
    let mut app_groups: std::collections::HashMap<String, AppEntry> = std::collections::HashMap::new();

    for (pid, process) in sys.processes() {
        let p_u32 = pid.as_u32();
        if p_u32 <= 4 || p_u32 == current_pid {
            continue;
        }

        let raw_name = process.name().to_string_lossy().to_string();
        let name_lower = raw_name.to_lowercase();

        // Never list ourselves as a capture source
        if name_lower.contains("virtual-mic-share") || name_lower.contains("virtual_mic_share") {
            continue;
        }

        let is_audio_active = active_audio_pids.contains(&p_u32);

        // Filter out system processes, Windows background services, and helper workers
        if ignored_names.contains(name_lower.as_str()) {
            continue;
        }

        if !is_audio_active {
            if name_lower.starts_with("crashpad")
                || name_lower.contains("crash_handler")
                || name_lower.contains(".service")
                || name_lower == "cmd.exe"
                || name_lower == "powershell.exe"
                || name_lower == "conhost.exe"
                || name_lower == "wsl.exe"
                || name_lower == "wslhost.exe"
            {
                continue;
            }
            if process.exe().is_none() {
                continue;
            }
        }

        let app_key = name_lower.clone();
        let friendly_name = format_friendly_app_name(&raw_name);
        let exe_name = process
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| raw_name.clone());

        match app_groups.get_mut(&app_key) {
            Some(existing) => {
                // If this process has active audio, prioritize its PID so process loopback attaches to sound
                if is_audio_active && !existing.is_audio_active {
                    existing.pid = p_u32;
                    existing.is_audio_active = true;
                } else if !existing.is_audio_active && p_u32 < existing.pid {
                    // Default to lowest PID (usually main application root)
                    existing.pid = p_u32;
                }
            }
            None => {
                app_groups.insert(
                    app_key,
                    AppEntry {
                        pid: p_u32,
                        name: friendly_name,
                        exe_name,
                        is_audio_active,
                    },
                );
            }
        }
    }

    let mut apps: Vec<AppEntry> = app_groups.into_values().collect();

    // Sort: active audio sessions first, then by name alphabetically
    apps.sort_by(|a, b| {
        b.is_audio_active
            .cmp(&a.is_audio_active)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_discovery() {
        let lists = get_device_lists().unwrap();
        println!("Found {} microphones", lists.microphones.len());
        println!("Found {} virtual outputs", lists.virtual_outputs.len());
        println!("Cable installed: {}", lists.cable_installed);
        assert!(!lists.microphones.is_empty());
        assert!(!lists.virtual_outputs.is_empty());
    }

    #[test]
    fn test_apps_discovery() {
        let apps = list_audio_applications().unwrap();
        println!("Found {} running apps", apps.len());
        for app in apps.iter().take(5) {
            println!("App: {} (PID: {}, audio: {})", app.name, app.pid, app.is_audio_active);
        }
    }
}
