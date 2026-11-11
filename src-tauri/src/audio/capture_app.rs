use crate::audio::{target_wave_format, CaptureSource, SharedDspState};
use rtrb::Producer;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use wasapi::{AudioClient, DeviceEnumerator, Direction, StreamMode};

pub fn run_app_capture(
    source: CaptureSource,
    dsp_state: Arc<SharedDspState>,
    mut app_producer: Producer<f32>,
) {
    let _ = wasapi::initialize_mta();

    let desired_format = target_wave_format();
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 100_000, // 10ms
    };

    let mut audio_client = match source {
        CaptureSource::Application { pid, include_tree } => {
            log::info!("Thread 2 [App Capture]: Initializing process loopback for PID {pid} (include_tree={include_tree})");
            // NOTE: Do NOT call get_mixformat() or get_device_period() on a process loopback client;
            // in wasapi 0.26.0 those return Not Implemented for process loopback.
            match AudioClient::new_application_loopback_client(pid, include_tree) {
                Ok(client) => client,
                Err(e) => {
                    log::error!("Thread 2 [App Capture]: Failed to create application loopback client for PID {pid}: {e}");
                    return;
                }
            }
        }
        CaptureSource::GlobalHeadphones => {
            log::info!("Thread 2 [App Capture]: Initializing global headphone render loopback");
            let enumerator = match DeviceEnumerator::new() {
                Ok(e) => e,
                Err(e) => {
                    log::error!("Thread 2 [App Capture]: Failed to create DeviceEnumerator: {e}");
                    return;
                }
            };
            let render_device = match enumerator.get_default_device(&Direction::Render) {
                Ok(dev) => dev,
                Err(e) => {
                    log::error!("Thread 2 [App Capture]: Failed to get default render device: {e}");
                    return;
                }
            };
            let name = render_device.get_friendlyname().unwrap_or_else(|_| "Default Render".into());
            log::info!("Thread 2 [App Capture]: Global loopback target: '{name}'");
            match render_device.get_iaudioclient() {
                Ok(client) => client,
                Err(e) => {
                    log::error!("Thread 2 [App Capture]: Failed to get IAudioClient for render loopback: {e}");
                    return;
                }
            }
        }
    };

    // Both modes initialize with Direction::Capture and StreamMode::EventsShared
    if let Err(e) = audio_client.initialize_client(&desired_format, &Direction::Capture, &mode) {
        log::error!("Thread 2 [App Capture]: Failed to initialize client: {e}");
        return;
    }

    let h_event = match audio_client.set_get_eventhandle() {
        Ok(h) => h,
        Err(e) => {
            log::error!("Thread 2 [App Capture]: Failed to get event handle: {e}");
            return;
        }
    };

    let capture_client = match audio_client.get_audiocaptureclient() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Thread 2 [App Capture]: Failed to get AudioCaptureClient: {e}");
            return;
        }
    };

    if let Err(e) = audio_client.start_stream() {
        log::error!("Thread 2 [App Capture]: Failed to start stream: {e}");
        return;
    }

    log::info!("Thread 2 [App Capture]: Streaming started successfully");

    let mut byte_buf = vec![0u8; 48000 * 8];

    while dsp_state.is_running.load(Ordering::Relaxed) {
        let _ = h_event.wait_for_event(100);

        if !dsp_state.is_running.load(Ordering::Relaxed) {
            break;
        }

        loop {
            let next_packet = match capture_client.get_next_packet_size() {
                Ok(Some(size)) => size,
                _ => break,
            };

            if next_packet == 0 {
                break;
            }

            match capture_client.read_from_device(&mut byte_buf) {
                Ok((frames, _info)) => {
                    if frames == 0 {
                        break;
                    }
                    let bytes_read = frames as usize * 8; // 2 channels * 4 bytes/sample
                    for chunk in byte_buf[..bytes_read].chunks_exact(4) {
                        let sample = f32::from_le_bytes(chunk.try_into().unwrap());
                        let _ = app_producer.push(sample);
                    }
                }
                Err(e) => {
                    log::warn!("Thread 2 [App Capture]: read_from_device error: {e}");
                    break;
                }
            }
        }
    }

    let _ = audio_client.stop_stream();
    log::info!("Thread 2 [App Capture]: Streaming stopped cleanly");
}
