pub mod capture_app;
pub mod capture_mic;
pub mod mixer_out;

use atomic_float::AtomicF32;
use rtrb::RingBuffer;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use wasapi::{SampleType, WaveFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CaptureSource {
    GlobalHeadphones,
    Application { pid: u32, include_tree: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub mic_device_id: String,
    pub virtual_output_id: String,
    pub source: CaptureSource,
}

pub struct SharedDspState {
    pub is_running: AtomicBool,
    pub app_volume: AtomicF32, // 0.0 to 2.0 (1.0 = 0 dB unity gain)
    pub mic_volume: AtomicF32, // 0.0 to 2.0 (1.0 = 0 dB unity gain)
    pub app_normalize: AtomicBool, // Automatic loudness normalization for application audio
}

impl Default for SharedDspState {
    fn default() -> Self {
        Self {
            is_running: AtomicBool::new(false),
            app_volume: AtomicF32::new(1.0),
            mic_volume: AtomicF32::new(1.0),
            app_normalize: AtomicBool::new(true),
        }
    }
}

pub fn target_wave_format() -> WaveFormat {
    // 32 bits allocated, 32 valid bits, Float32, 48000 Hz, 2 channels (Stereo)
    WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None)
}

pub struct AudioEngine {
    pub dsp_state: Arc<SharedDspState>,
    worker_handles: Vec<JoinHandle<()>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            dsp_state: Arc::new(SharedDspState::default()),
            worker_handles: Vec::new(),
        }
    }

    pub fn start(&mut self, config: StreamConfig) -> Result<(), String> {
        // If already running, stop previous threads cleanly first
        if self.dsp_state.is_running.load(Ordering::SeqCst) {
            let _ = self.stop();
        }

        self.dsp_state.is_running.store(true, Ordering::SeqCst);

        // Ring buffers: 48000 frames * 2 channels = 96,000 f32 samples (1 second capacity)
        let (mic_producer, mic_consumer) = RingBuffer::<f32>::new(48000 * 2);
        let (app_producer, app_consumer) = RingBuffer::<f32>::new(48000 * 2);

        // Thread 1: Mic Capture
        let mic_dsp = Arc::clone(&self.dsp_state);
        let mic_dev_id = config.mic_device_id.clone();
        let mic_handle = std::thread::Builder::new()
            .name("MicCaptureWorker".into())
            .spawn(move || {
                capture_mic::run_mic_capture(mic_dev_id, mic_dsp, mic_producer);
            })
            .map_err(|e| format!("Failed to spawn Thread 1 (Mic Capture): {e}"))?;

        // Thread 2: App / Global Capture
        let app_dsp = Arc::clone(&self.dsp_state);
        let source_clone = config.source.clone();
        let app_handle = std::thread::Builder::new()
            .name("AppCaptureWorker".into())
            .spawn(move || {
                capture_app::run_app_capture(source_clone, app_dsp, app_producer);
            })
            .map_err(|e| format!("Failed to spawn Thread 2 (App Capture): {e}"))?;

        // Thread 3: DSP Mixer & Virtual Output
        let mixer_dsp = Arc::clone(&self.dsp_state);
        let out_dev_id = config.virtual_output_id.clone();
        let mixer_handle = std::thread::Builder::new()
            .name("MixerOutputWorker".into())
            .spawn(move || {
                mixer_out::run_mixer_output(out_dev_id, mixer_dsp, mic_consumer, app_consumer);
            })
            .map_err(|e| format!("Failed to spawn Thread 3 (Mixer Output): {e}"))?;

        self.worker_handles = vec![mic_handle, app_handle, mixer_handle];
        log::info!("AudioEngine: All 3 audio workers spawned and running");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.dsp_state.is_running.store(false, Ordering::SeqCst);

        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }

        log::info!("AudioEngine: All worker threads cleanly joined");
        Ok(())
    }

    pub fn set_app_volume(&self, volume: f32) -> Result<(), String> {
        let clamped = volume.clamp(0.0, 2.0);
        self.dsp_state.app_volume.store(clamped, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_mic_volume(&self, volume: f32) -> Result<(), String> {
        let clamped = volume.clamp(0.0, 2.0);
        self.dsp_state.mic_volume.store(clamped, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_app_normalize(&self, enabled: bool) -> Result<(), String> {
        self.dsp_state.app_normalize.store(enabled, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_clamping() {
        let engine = AudioEngine::new();
        engine.set_app_volume(0.5).unwrap();
        assert_eq!(engine.dsp_state.app_volume.load(Ordering::Relaxed), 0.5);

        engine.set_app_volume(3.0).unwrap();
        assert_eq!(engine.dsp_state.app_volume.load(Ordering::Relaxed), 2.0);

        engine.set_mic_volume(-1.0).unwrap();
        assert_eq!(engine.dsp_state.mic_volume.load(Ordering::Relaxed), 0.0);
    }

    #[test]
    fn test_target_format() {
        let fmt = target_wave_format();
        assert_eq!(fmt.get_samplespersec(), 48000);
        assert_eq!(fmt.get_nchannels(), 2);
    }
}
