# AudioShare 🎙️🔊

**AudioShare** is a high-performance Windows desktop application built with **Rust (2024 Edition, MSRV 1.88+)**, **Tauri v2**, and **React + TypeScript**. It allows you to seamlessly mix your physical microphone with per-application or global system audio in real time, streaming the combined audio into a virtual microphone (such as VB-Audio Virtual Cable) for Discord, Zoom, OBS, Teams, or games.

Unlike basic loopback tools, AudioShare isolates audio from specific processes using Windows WASAPI Process Loopback, so you can share YouTube, Spotify, or gameplay sound without echoing your teammates' voices back to them.

---

## ✨ Features

- **Per-Application Audio Capture**: Capture audio exclusively from a selected application (e.g. Chrome, Spotify, games) using Windows 10/11 WASAPI Process Loopback (`include_tree: true` to capture all child subprocesses).
- **All System Audio (Global Loopback)**: Alternative mode to capture everything playing through your default speakers or headphones.
- **Lock-Free Multi-Threaded Audio Engine**:
  - **Thread 1 (Mic Capture)**: Low-latency physical microphone capture at 48kHz, 2-channel stereo float (`f32`).
  - **Thread 2 (App Capture)**: Process loopback or default render loopback.
  - **Thread 3 (DSP Mixer & Output)**: Consumes from lock-free `rtrb` ring buffers with real-time atomic volume sliders (`0%` to `200%`).
- **Real-Time Automatic Loudness Normalizer (AGC)**:
  - Smooth 25ms attack / 1.0s release envelope tracking stabilizes application sound to a broadcast standard reference level (~0.65 peak).
  - Eliminates sudden volume dipping, pumping, or inaudible dialogues without touching your natural microphone voice.
  - Gated noise floor prevents amplifying background silence.
- **Drift & Latency Guard**: Continuously checks ring buffer levels and trims old excess samples down to ~25ms if drift exceeds 100ms, ensuring zero latency buildup over hours of streaming.
- **Transparent Soft-Knee Tanh Limiter**: Smoothly limits output peaks to prevent digital distortion and harsh clipping.
- **Clean One-Entry-Per-App UI**: Automatically groups and deduplicates multi-process applications (Chrome, Discord, Steam) with instant search and active audio detection.

---

## 🎧 How It Works: VB-Audio Virtual Cable Routing

AudioShare acts as the mixer between your audio sources and your VoIP/streaming software via VB-Audio Virtual Cable:

```
[ Your Physical Mic ] ────┐
                          ├──> [ AudioShare App ] ───> CABLE Input (VB-Audio Virtual Cable)
[ Target App (e.g. Game) ]──┘                                   │
                                                               │ (Virtual Cable)
                                                               ▼
[ Discord / Zoom / Games ]  <──── Set Input Device (Mic) to: CABLE Output (VB-Audio Virtual Cable)
```

1. **Inside AudioShare**:
   - AudioShare routes the mixed sound into the **`CABLE Input (VB-Audio Virtual Cable)`** playback device.
2. **Inside Destination Software (Discord, Zoom, OBS, Teams, Games)**:
   - Navigate to **Voice / Audio Settings**.
   - Change your **Input Device (Microphone)** to: **`CABLE Output (VB-Audio Virtual Cable)`**.
   - Your audience or teammates will now hear your voice and the shared application audio together in crystal clear quality!

---

## 🚀 Installation & Setup

### Option A: Pre-built Binaries (Recommended)
1. Download either the **Portable Executable** (`virtual-mic-share.exe`) or the **NSIS Setup Installer** (`Virtual Mic Share_0.1.0_x64-setup.exe`) from the repository releases or build artifacts.
2. Ensure you have installed [VB-Audio Virtual Cable](https://vb-audio.com/Cable/) (free, standard virtual audio driver for Windows).
3. Run `virtual-mic-share.exe`.

### Option B: Build from Source

#### Prerequisites
- **Windows 10 (build 20348+) or Windows 11**
- **Rust toolchain (1.88+)**: Install via [rustup.rs](https://rustup.rs)
- **Node.js (v18+) & npm**: Install from [nodejs.org](https://nodejs.org)
- **VB-Audio Virtual Cable Driver**: Download and install from [vb-audio.com/Cable](https://vb-audio.com/Cable/)

#### Steps

1. **Clone the repository**:
   ```bash
   git clone https://github.com/KappinChromie/AudioShare.git
   cd AudioShare
   ```

2. **Install frontend dependencies**:
   ```bash
   npm install
   ```

3. **Run in Development Mode**:
   ```bash
   npm run tauri dev
   ```

4. **Run Unit and Integration Tests**:
   ```bash
   cd src-tauri
   cargo test -- --nocapture
   cd ..
   ```

5. **Build for Production (Release `.exe` & NSIS installer)**:
   ```bash
   npm run tauri build
   ```
   The built binaries will be located at:
   - Portable `.exe`: `src-tauri/target/release/virtual-mic-share.exe`
   - Setup installer: `src-tauri/target/release/bundle/nsis/Virtual Mic Share_0.1.0_x64-setup.exe`

---

## 🎛️ Usage Guide

1. **Select Microphone**: Choose your physical headset or standalone USB microphone from the dropdown and set the desired gain (default 100%, double-click to reset).
2. **Choose Audio Source**:
   - **Specific Application**: Filter and select any running app (e.g. *Discord*, *Spotify*, *Google Chrome*, or a game). Processes currently playing sound are marked with `🔊 [Active Audio]`.
   - **All System Audio**: Captures all system sound currently playing through your default playback device.
3. **Set Streamed Audio Volume**: Adjust the volume slider to balance how loud the application audio plays relative to your voice.
4. **Auto-Normalize**: Keep *Auto-Normalize Audio* checked to ensure steady, broadcast-ready volume levels without dipping.
5. **Start Audio Share**: Click the green **Start Audio Share** button. The indicator will turn to a pulsing **STREAMING** status.

---

## 🛠️ Tech Stack & Architecture

- **Backend**: Rust 2024 Edition, `tauri 2.2`, `wasapi 0.26.0`, `rtrb 0.3` (lock-free ring buffers), `atomic_float 1.1`, `sysinfo 0.38`.
- **Frontend**: React 19, TypeScript, Vite, Lucide Icons.
- **Thread Safety**: Strict Windows MTA (`wasapi::initialize_mta()`) initialization per thread. Audio client COM pointers never cross thread boundaries; data transfer occurs exclusively through lock-free ring buffers.

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
