use crate::audio::{target_wave_format, SharedDspState};
use rtrb::Consumer;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use wasapi::{Device, DeviceEnumerator, Direction, StreamMode};

fn find_virtual_cable_or_default(enumerator: &DeviceEnumerator) -> Result<Device, String> {
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| format!("Failed to get render device collection: {e}"))?;

    for dev_res in &collection {
        if let Ok(dev) = dev_res {
            if let Ok(name) = dev.get_friendlyname() {
                let name_lower = name.to_lowercase();
                if name_lower.contains("cable input")
                    || name_lower.contains("cable in")
                    || name_lower.contains("blackhole")
                    || name_lower.contains("virtual")
                {
                    log::info!("Thread 3 [Mixer Out]: Found virtual render device: '{name}'");
                    return Ok(dev);
                }
            }
        }
    }

    log::warn!("Thread 3 [Mixer Out]: No virtual cable output detected, falling back to default render device");
    enumerator
        .get_default_device(&Direction::Render)
        .map_err(|e| format!("Failed to get default render device: {e}"))
}

/// Transparent soft-clipper: linear between -0.9 and +0.9, soft-knee above 0.9
#[inline]
pub fn apply_soft_clipper(summed: f32) -> f32 {
    if summed.abs() <= 0.9 {
        summed
    } else {
        summed.signum() * (0.9 + 0.1 * ((summed.abs() - 0.9) * 10.0).tanh())
    }
}

/// Latency / Buffer Drift Guard:
/// If the ring buffer accumulated more than `max_threshold` samples (e.g. 9600 = ~100ms stereo at 48kHz),
/// discard the oldest excess samples down to `target_level` (e.g. 2400 = ~25ms stereo at 48kHz).
pub fn trim_buffer_drift(consumer: &mut Consumer<f32>, max_threshold: usize, target_level: usize) {
    let slots = consumer.slots();
    if slots > max_threshold {
        let discard_count = slots.saturating_sub(target_level);
        // Ensure even count to maintain stereo 2-channel L/R frame alignment
        let aligned_discard = (discard_count / 2) * 2;
        if aligned_discard > 0 {
            if let Ok(chunk) = consumer.read_chunk(aligned_discard) {
                chunk.commit_all();
            }
        }
    }
}

/// Real-time automatic loudness normalizer for application audio.
/// Tracks stereo envelope with smooth attack and release to eliminate sudden dips or pumping,
/// bringing quiet dialogues/videos up to a broadcast standard reference level (~0.65 peak)
/// while gating out background silence.
#[derive(Debug, Clone)]
pub struct AppNormalizer {
    pub envelope: f32,
    pub current_gain: f32,
    pub target_level: f32,
    pub attack_coeff: f32,
    pub release_coeff: f32,
    pub gain_smooth_coeff: f32,
    pub noise_gate_threshold: f32,
}

impl Default for AppNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl AppNormalizer {
    pub fn new() -> Self {
        let sample_rate = 48000.0f32;
        let attack_time = 0.025f32; // 25ms fast attack
        let release_time = 1.0f32;  // 1.0s slow release prevents dips/pumping
        let gain_smooth_time = 0.050f32; // 50ms gain smoothing

        Self {
            envelope: 0.0,
            current_gain: 1.0,
            target_level: 0.65, // Standardized normalized peak level
            attack_coeff: 1.0 - (-1.0 / (attack_time * sample_rate)).exp(),
            release_coeff: 1.0 - (-1.0 / (release_time * sample_rate)).exp(),
            gain_smooth_coeff: 1.0 - (-1.0 / (gain_smooth_time * sample_rate)).exp(),
            noise_gate_threshold: 0.003, // ~ -50 dBFS gate floor
        }
    }

    /// Process a stereo frame (Left and Right samples).
    /// Tracks maximum absolute amplitude so stereo imaging and panning are preserved.
    #[inline]
    pub fn process_frame(&mut self, left: f32, right: f32) -> (f32, f32) {
        let peak = left.abs().max(right.abs());

        // Update envelope with asymmetric attack/release
        if peak > self.envelope {
            self.envelope += self.attack_coeff * (peak - self.envelope);
        } else {
            self.envelope += self.release_coeff * (peak - self.envelope);
        }

        // Calculate desired gain only when signal is above noise floor
        let desired_gain = if self.envelope > self.noise_gate_threshold {
            (self.target_level / self.envelope).clamp(0.5, 4.0)
        } else {
            1.0 // In silence, smoothly return to unity
        };

        // Smooth gain to eliminate any pumping or clicking
        self.current_gain += self.gain_smooth_coeff * (desired_gain - self.current_gain);

        (left * self.current_gain, right * self.current_gain)
    }
}

pub fn run_mixer_output(
    virtual_output_id: String,
    dsp_state: Arc<SharedDspState>,
    mut mic_consumer: Consumer<f32>,
    mut app_consumer: Consumer<f32>,
) {
    let _ = wasapi::initialize_mta();

    let enumerator = match DeviceEnumerator::new() {
        Ok(e) => e,
        Err(e) => {
            log::error!("Thread 3 [Mixer Out]: Failed to create DeviceEnumerator: {e}");
            return;
        }
    };

    let device = if !virtual_output_id.is_empty() && virtual_output_id != "default" {
        match enumerator.get_device(&virtual_output_id) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Thread 3 [Mixer Out]: Output device '{virtual_output_id}' not found ({e}), falling back");
                match find_virtual_cable_or_default(&enumerator) {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("Thread 3 [Mixer Out]: Failed to find render device: {e}");
                        return;
                    }
                }
            }
        }
    } else {
        match find_virtual_cable_or_default(&enumerator) {
            Ok(d) => d,
            Err(e) => {
                log::error!("Thread 3 [Mixer Out]: Failed to find render device: {e}");
                return;
            }
        }
    };

    let dev_name = device.get_friendlyname().unwrap_or_else(|_| "Unknown Output".into());
    log::info!("Thread 3 [Mixer Out]: Rendering to endpoint '{dev_name}'");

    let mut audio_client = match device.get_iaudioclient() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Thread 3 [Mixer Out]: Failed to get IAudioClient: {e}");
            return;
        }
    };

    let desired_format = target_wave_format();
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 100_000, // 10ms
    };

    if let Err(e) = audio_client.initialize_client(&desired_format, &Direction::Render, &mode) {
        log::error!("Thread 3 [Mixer Out]: Failed to initialize client: {e}");
        return;
    }

    let h_event = match audio_client.set_get_eventhandle() {
        Ok(h) => h,
        Err(e) => {
            log::error!("Thread 3 [Mixer Out]: Failed to get event handle: {e}");
            return;
        }
    };

    let render_client = match audio_client.get_audiorenderclient() {
        Ok(r) => r,
        Err(e) => {
            log::error!("Thread 3 [Mixer Out]: Failed to get AudioRenderClient: {e}");
            return;
        }
    };

    // Pre-fill the render buffer with silence before calling start_stream()
    let buffer_frame_count = match audio_client.get_buffer_size() {
        Ok(size) => size as usize,
        Err(e) => {
            log::error!("Thread 3 [Mixer Out]: Failed to get buffer size: {e}");
            return;
        }
    };

    let silence_bytes = vec![0u8; buffer_frame_count * 8];
    if let Err(e) = render_client.write_to_device(buffer_frame_count, &silence_bytes, None) {
        log::error!("Thread 3 [Mixer Out]: Failed to pre-fill render buffer: {e}");
        return;
    }

    if let Err(e) = audio_client.start_stream() {
        log::error!("Thread 3 [Mixer Out]: Failed to start stream: {e}");
        return;
    }

    log::info!("Thread 3 [Mixer Out]: Mixer output stream started successfully");

    let mut byte_buffer = Vec::with_capacity(buffer_frame_count * 8);
    let mut app_normalizer = AppNormalizer::new();

    while dsp_state.is_running.load(Ordering::Relaxed) {
        let _ = h_event.wait_for_event(100);

        if !dsp_state.is_running.load(Ordering::Relaxed) {
            break;
        }

        // Latency / Buffer Drift Guard:
        // Discard excess old samples down to ~2400 samples (~25ms) if queue > 9600 samples (~100ms)
        trim_buffer_drift(&mut mic_consumer, 9600, 2400);
        trim_buffer_drift(&mut app_consumer, 9600, 2400);

        let available_frames = match audio_client.get_available_space_in_frames() {
            Ok(f) => f as usize,
            Err(e) => {
                log::warn!("Thread 3 [Mixer Out]: get_available_space_in_frames error: {e}");
                continue;
            }
        };

        if available_frames == 0 {
            continue;
        }

        let mic_gain = dsp_state.mic_volume.load(Ordering::Relaxed);
        let app_gain = dsp_state.app_volume.load(Ordering::Relaxed);
        let normalize_enabled = dsp_state.app_normalize.load(Ordering::Relaxed);

        byte_buffer.clear();

        for _ in 0..available_frames {
            let mic_l = mic_consumer.pop().unwrap_or(0.0) * mic_gain;
            let mic_r = mic_consumer.pop().unwrap_or(0.0) * mic_gain;

            let raw_app_l = app_consumer.pop().unwrap_or(0.0);
            let raw_app_r = app_consumer.pop().unwrap_or(0.0);

            let (norm_app_l, norm_app_r) = if normalize_enabled {
                app_normalizer.process_frame(raw_app_l, raw_app_r)
            } else {
                (raw_app_l, raw_app_r)
            };

            let app_l = norm_app_l * app_gain;
            let app_r = norm_app_r * app_gain;

            let out_l = apply_soft_clipper(mic_l + app_l);
            let out_r = apply_soft_clipper(mic_r + app_r);

            byte_buffer.extend_from_slice(&out_l.to_le_bytes());
            byte_buffer.extend_from_slice(&out_r.to_le_bytes());
        }

        if let Err(e) = render_client.write_to_device(available_frames, &byte_buffer, None) {
            log::warn!("Thread 3 [Mixer Out]: write_to_device error: {e}");
        }
    }

    let _ = audio_client.stop_stream();
    log::info!("Thread 3 [Mixer Out]: Mixer output stream stopped cleanly");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soft_clipper_linearity() {
        assert_eq!(apply_soft_clipper(0.0), 0.0);
        assert_eq!(apply_soft_clipper(0.5), 0.5);
        assert_eq!(apply_soft_clipper(-0.5), -0.5);
        assert_eq!(apply_soft_clipper(0.9), 0.9);
        assert_eq!(apply_soft_clipper(-0.9), -0.9);
    }

    #[test]
    fn test_soft_clipper_saturation() {
        let clipped_pos = apply_soft_clipper(1.5);
        assert!(clipped_pos > 0.9 && clipped_pos <= 1.0);

        let clipped_extreme = apply_soft_clipper(10.0);
        assert!(clipped_extreme <= 1.0);

        let clipped_neg = apply_soft_clipper(-1.5);
        assert!(clipped_neg < -0.9 && clipped_neg >= -1.0);
    }

    #[test]
    fn test_trim_buffer_drift() {
        use rtrb::RingBuffer;
        let (mut p, mut c) = RingBuffer::<f32>::new(20000);
        for i in 0..10000 {
            p.push(i as f32).unwrap();
        }
        assert_eq!(c.slots(), 10000);

        trim_buffer_drift(&mut c, 9600, 2400);
        assert_eq!(c.slots(), 2400);
        assert_eq!(c.pop().unwrap(), 7600.0);
    }

    #[test]
    fn test_normalizer_quiet_signal_boost() {
        let mut norm = AppNormalizer::new();
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for _ in 0..48000 {
            let res = norm.process_frame(0.1, 0.1);
            out_l = res.0;
            out_r = res.1;
        }
        assert!(out_l > 0.3 && out_l <= 0.7);
        assert_eq!(out_l, out_r);
    }

    #[test]
    fn test_normalizer_silence_gating() {
        let mut norm = AppNormalizer::new();
        for _ in 0..48000 {
            norm.process_frame(0.0, 0.0);
        }
        assert!((norm.current_gain - 1.0).abs() < 0.05);
    }
}
