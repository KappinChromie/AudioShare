# AudioShare v0.1.0 — Initial Release 🎙️🔊

High-performance Windows desktop application built with Rust (MSRV 1.88+), Tauri v2, and React + TypeScript to mix microphone audio with per-application or system audio and stream into a virtual microphone (e.g., VB-Audio Virtual Cable) for Discord, Zoom, OBS, Teams, or games.

### 🌟 Key Features
- **Per-Process Audio Capture**: Capture audio exclusively from a selected application using Windows 10/11 WASAPI Process Loopback (`include_tree: true`) — share games, YouTube, or music without echoing your Discord friends back to themselves.
- **All System Audio Mode**: Capture everything playing through default speakers/headphones.
- **Automatic Loudness Normalizer (AGC)**: Smooth 25ms attack / 1.0s release envelope tracker stabilizes application sound to a standard reference level without dipping, pumping, or altering microphone audio.
- **Latency & Buffer Drift Guard**: Automatically trims old excess samples down to ~25ms if buffer drift exceeds 100ms over long streaming sessions.
- **Soft-Knee Tanh Limiter**: Smooth transparent limiting to eliminate clipping distortion.
- **Deduplicated App Picker**: Clean UI listing one entry per app with instant search and `🔊 [Active Audio]` detection.

### 📦 Included Assets
- **`Virtual Mic Share_0.1.0_x64-setup.exe`**: Full Windows NSIS setup installer.
- **`virtual-mic-share.exe`**: Standalone portable executable.

### 🎧 Audio Routing Setup
1. In AudioShare, select **`CABLE Input (VB-Audio Virtual Cable)`** as the output device.
2. In Discord / Zoom / OBS / Game, set your **Input Device (Microphone)** to **`CABLE Output (VB-Audio Virtual Cable)`**.
*(Requires [VB-Audio Virtual Cable](https://vb-audio.com/Cable/) installed on Windows).*
