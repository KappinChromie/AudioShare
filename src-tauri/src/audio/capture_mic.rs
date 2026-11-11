use crate::audio::{target_wave_format, SharedDspState};
use rtrb::Producer;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use wasapi::{DeviceEnumerator, Direction, StreamMode};

pub fn run_mic_capture(
    mic_device_id: String,
    dsp_state: Arc<SharedDspState>,
    mut mic_producer: Producer<f32>,
) {
    let _ = wasapi::initialize_mta();

    let enumerator = match DeviceEnumerator::new() {
        Ok(e) => e,
        Err(err) => {
            log::error!("Thread 1 [Mic Capture]: Failed to create DeviceEnumerator: {err}");
            return;
        }
    };

    let device = if !mic_device_id.is_empty() && mic_device_id != "default" {
        match enumerator.get_device(&mic_device_id) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Thread 1 [Mic Capture]: Device '{mic_device_id}' not found ({e}), falling back to default capture device");
                match enumerator.get_default_device(&Direction::Capture) {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("Thread 1 [Mic Capture]: Failed to get default capture device: {e}");
                        return;
                    }
                }
            }
        }
    } else {
        match enumerator.get_default_device(&Direction::Capture) {
            Ok(d) => d,
            Err(e) => {
                log::error!("Thread 1 [Mic Capture]: Failed to get default capture device: {e}");
                return;
            }
        }
    };

    let dev_name = device.get_friendlyname().unwrap_or_else(|_| "Unknown".into());
    log::info!("Thread 1 [Mic Capture]: Selected microphone '{dev_name}'");

    let mut audio_client = match device.get_iaudioclient() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Thread 1 [Mic Capture]: Failed to get IAudioClient: {e}");
            return;
        }
    };

    let desired_format = target_wave_format();
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 100_000, // 10ms buffer
    };

    if let Err(e) = audio_client.initialize_client(&desired_format, &Direction::Capture, &mode) {
        log::error!("Thread 1 [Mic Capture]: Failed to initialize client: {e}");
        return;
    }

    let h_event = match audio_client.set_get_eventhandle() {
        Ok(h) => h,
        Err(e) => {
            log::error!("Thread 1 [Mic Capture]: Failed to set event handle: {e}");
            return;
        }
    };

    let capture_client = match audio_client.get_audiocaptureclient() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Thread 1 [Mic Capture]: Failed to get AudioCaptureClient: {e}");
            return;
        }
    };

    if let Err(e) = audio_client.start_stream() {
        log::error!("Thread 1 [Mic Capture]: Failed to start stream: {e}");
        return;
    }

    log::info!("Thread 1 [Mic Capture]: Streaming started successfully");

    // Preallocated byte buffer for reading frames (up to 48000 frames * 8 bytes)
    let mut byte_buf = vec![0u8; 48000 * 8];

    while dsp_state.is_running.load(Ordering::Relaxed) {
        // Wait on event with 100ms timeout to allow checking is_running
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
                        // Push into ring buffer. If full, drop sample without blocking.
                        let _ = mic_producer.push(sample);
                    }
                }
                Err(e) => {
                    log::warn!("Thread 1 [Mic Capture]: read_from_device error: {e}");
                    break;
                }
            }
        }
    }

    let _ = audio_client.stop_stream();
    log::info!("Thread 1 [Mic Capture]: Streaming stopped cleanly");
}
