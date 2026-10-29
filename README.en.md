<div align="center">
  <img src="docs/brand/banner.svg" alt="Audio Pulse — studio-grade sound from any headphones" width="100%" />
</div>

<h3 align="center">Your headphones, tuned to your ears — everywhere on your computer.</h3>

<p align="center">
  A low-latency DSP engine for every sound the system plays: 10-band EQ, parametric AutoEQ,<br/>
  bass synthesis, ISO 226 loudness, loud sound reduction, dynamic HRTF and local AI ear profiling.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-5CF2D4?style=flat-square" alt="GPL-3.0-or-later" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-73A1FF?style=flat-square" alt="Windows, macOS and Linux" />
  <img src="https://img.shields.io/badge/built%20with-Tauri%202%20%C2%B7%20Rust%20%C2%B7%20React-B880FF?style=flat-square" alt="Tauri 2, Rust and React" />
  <img src="https://img.shields.io/badge/AI-100%25%20local-9FB0C8?style=flat-square" alt="AI runs locally" />
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#screenshots">Screenshots</a> ·
  <a href="#build-from-source">Build</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="docs/reference.pt-BR.md">Technical reference (pt-BR)</a> ·
  <a href="README.md">Português</a>
</p>

[![Audio Pulse dashboard](docs/screenshots/dashboard.png)](docs/screenshots/dashboard.png)

## Why Audio Pulse

Most audio tools only change the app you are listening to. Audio Pulse sits
between the operating system and your headphones, so the same chain processes
music, games, video calls and browser tabs — wired or Bluetooth, no vendor
driver and no per-player plugin.

The audio path is written in Rust and never allocates while sound is flowing;
the React interface only draws state and publishes parameters through an
`ArcSwap`, so dragging a slider cannot stall the audio thread. Everything runs
on your machine: no account, no telemetry, no cloud processing.

## Features

| Sound | Personalization | Desktop integration |
| --- | --- | --- |
| **10-band graphic EQ** — 31 Hz to 16 kHz, ±12 dB, with a live response curve and draggable nodes | **Ear profiling** — local depth model estimates concha geometry and proposes a 2–8 kHz correction | **System-wide routing** — virtual cable plus automatic default-endpoint switch on Windows |
| **Parametric AutoEQ/OPRA** — up to 16 PK/LS/HS filters, TXT or JSON import, automatic preamp | **Presets** — 12 listening modes and 20 measured headphone profiles | **Volume sync** — the app slider and the Windows volume controller are one control |
| **Adaptive EQ** — 5 octave bands every 20 ms, ±3 dB, predictive headroom; deterministic, not a black box | **Custom presets** — create, export and import JSON profiles | **Per-device profiles** — apply a preset when a specific output starts |
| **Bass Boost** — low shelf plus missing-fundamental harmonic synthesis | **Ear profile stage** — adds to the preset and AutoEQ; one-click **Restore defaults** | **Tray, shortcuts, autostart** — control playback without opening the window |
| **Dynamics** — RMS compressor with a sidechain low-cut and a 5 ms true-peak limiter | | **Wireless battery** — HID charge reports from USB receivers on Windows |
| **Loudness** — ISO 226 compensation that follows the master volume | | **Real meters** — 56-band spectrum, L/R levels, gain reduction, CPU, latency and xruns |
| **Loud sound reduction** — slow stereo compressor after the volume; adjustable intensity and live engagement | | |
| **Spatial audio** — binaural, dynamic HRTF and 7.1 upmix; crossfeed for headphones | | **Interface** — dark/light, English and pt-BR, responsive |

## Screenshots

Click any screenshot to open it at full resolution.

| Equalizer | Presets |
| --- | --- |
| <a href="docs/screenshots/equalizer.png"><img src="docs/screenshots/equalizer.png" alt="Equalizer page" /></a> | <a href="docs/screenshots/presets.png"><img src="docs/screenshots/presets.png" alt="Presets page" /></a> |
| 10 bands, tone, Bass Boost, loudness, crossfeed and AutoEQ in one chain. | Mode presets, measured headphone profiles, custom profiles and AutoEQ/OPRA import. |

| Spatial audio | Dynamics |
| --- | --- |
| <a href="docs/screenshots/spatial.png"><img src="docs/screenshots/spatial.png" alt="Spatial audio page" /></a> | <a href="docs/screenshots/dynamics.png"><img src="docs/screenshots/dynamics.png" alt="Dynamics page" /></a> |
| Binaural, dynamic HRTF and virtual surround with width and room controls. | RMS compressor and true-peak limiter with live transfer curves. |

| Ear profiling | Settings |
| --- | --- |
| <a href="docs/screenshots/ear.png"><img src="docs/screenshots/ear.png" alt="Ear profiling page" /></a> | <a href="docs/screenshots/settings.png"><img src="docs/screenshots/settings.png" alt="Settings page" /></a> |
| Guided crop, local depth analysis, confidence gates and a reversible AutoEQ proposal. | Devices, routing, buffer, theme, language and global shortcuts. |

## How it works

```text
Any application ──► virtual cable ──► Audio Pulse ──► your headphones
                                          │
subsonic → preamp → AutoEQ → EQ 10 → tone → bass → spatial
        → crossfeed → adaptive EQ → compressor → volume → loudness
        → loud sound reduction → limiter
```

The first stage is a 4th-order 20 Hz Butterworth high-pass: it removes DC and
infrasound that displace the diaphragm without being audible — felt exactly as
pressure in the ear. It sits below the 31 Hz EQ band, so it does not change the
bass you hear.

- **One process, one audio thread.** CPAL owns the input and output streams for
  the whole session. Buffers, filters and spectra are pre-allocated; parameter
  changes swap an `ArcSwap` pointer and rebuild only the stages that changed.
- **Click-free preset changes.** Discrete changes ramp the output down for
  2 ms, apply the new coefficients in silence and ramp back over 30 ms.
- **Clock-drift correction.** Capture and playback may run at different rates;
  a ring buffer with drift tracking (±0.15 %) absorbs the difference and counts
  xruns instead of letting the queues grow.
- **Rust DSP crate.** [`crates/audio-pulse-dsp`](crates/audio-pulse-dsp) is pure
  DSP with no device dependency: RBJ biquads, bass synthesis, spatial renderer,
  adaptive EQ, compressor, limiter, ISO 226 loudness, loud sound reduction and
  spectrum analysis.

Built with Tauri 2, Rust, React 19, TypeScript, Vite 8, CPAL, `ringbuf`,
`arc-swap` and ONNX Runtime (`ort`, statically linked).

## Ear profiling, without the cloud

The **Ear** page guides you through a 512×512 crop of your ear, then runs
[Depth Anything V2 Small](https://huggingface.co/onnx-community/depth-anything-v2-small)
(fp32 ONNX, ~95 MB) on the local CPU. An adaptive zone search over the depth map
locates the concha, four geometry metrics drive a small parametric correction,
and three peaking filters in 2–8 kHz are proposed as a profile.

- The photo is decoded, cropped and analyzed **on your machine**; it is never
  uploaded and no network request is made during analysis.
- The model is downloaded automatically the first time the page opens, from
  Hugging Face, and verified against a pinned SHA-256 before use.
- The profile runs as its **own stage**, between AutoEQ and the graphic EQ: it
  adds to the preset and to AutoEQ instead of replacing them, and presets never
  carry or clear it.
- Applying a profile is always explicit, and **Remove profile** disables it
  without touching the rest of the chain. **Restore defaults** in Settings
  returns to the Music preset with an empty ear profile, keeping the Windows
  master volume.
- Low confidence, off-center crops, missing concha, dark or overexposed photos
  block application instead of guessing.

This is an **estimate**, not a measured HRTF: a 2D photo cannot capture ear
canal geometry, head shadow or individual pinna response. Treat the curve as a
starting point and compare with **Bypass**. The full method, thresholds and
limits are documented in the [technical reference](docs/reference.pt-BR.md#como-funciona-o-perfil-por-foto-da-orelha).

## Privacy

- No account, no analytics, no telemetry, no crash reporting.
- Audio never leaves the machine.
- Network access happens only when you request it: the depth model download on
  the Ear page, and the virtual-cable installer download on Windows/macOS.
  Both are checksum/signature verified.
- Configuration and presets are plain JSON on your disk. Nothing is synced.

## Build from source

### Requirements

| Platform | Toolchain |
| --- | --- |
| All | Node.js 20.19+ or 22.12+, npm, Rust stable via [rustup](https://rustup.rs) |
| Windows 10/11 | Visual Studio Build Tools with “Desktop development with C++”, Windows SDK, WebView2 runtime |
| macOS 11+ | Xcode Command Line Tools (`xcode-select --install`) |
| Linux | `build-essential`, `libssl-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libasound2-dev` (Debian/Ubuntu names) |

```bash
git clone https://github.com/DouglasGDev/audio-pulse.git
cd audio-pulse

npm install
npm run tauri dev      # desktop app with hot reload
npm run tauri build    # release binary + installer

cargo test --workspace # DSP, AutoEQ/OPRA parsers, resampler, config migration
npm run build          # type-check + production frontend
```

The release executable is written to `target/release/audio-pulse` (`.exe` on
Windows); installers land in `target/release/bundle/`.

`npm run dev` opens the interface in a browser with a **simulated backend**:
navigation and layout work, but no audio is processed. Use it for UI work, not
for listening tests.

### Routing system audio

Audio Pulse does not ship an audio driver. To process everything, route the
system through a virtual cable:

1. **Windows** — in *Settings → Audio*, click **Install virtual cable**. The app
   downloads the official VB-CABLE package, verifies its Authenticode signature,
   runs it (one UAC prompt) and selects `CABLE Output` as capture. While the
   route is active, Windows' default output is switched to `CABLE Input` and
   restored when you stop or quit. If you plug in headphones or change the
   default output, the route follows the new device and takes the cable back;
   if the headphones or cable disappear, it restarts on its own and shows a
   warning.
2. **macOS** — install BlackHole 2ch from the app, set it as the system output,
   then select it as capture and your headphones as output.
3. **Linux** — create a null sink and use its monitor as capture:

   ```bash
   pactl load-module module-null-sink sink_name=audiopulse \
     sink_properties=device.description=AudioPulse
   ```

No cable? The dashboard can play pink noise, a sweep or a WAV file straight
through the processed output, so you can evaluate the chain without routing.

## Keyboard shortcuts

| Action | Default |
| --- | --- |
| Bypass on/off | `Ctrl/Cmd + Alt + B` |
| Music / Cinema / Gaming / Podcast | `Ctrl/Cmd + Alt + 1/2/3/4` |
| Cycle spatial mode | `Ctrl/Cmd + Alt + S` |
| Volume up / down | `Ctrl/Cmd + Alt + Up/Down` |
| Open window | `Ctrl/Cmd + Alt + O` |

All shortcuts can be rebound in *Settings → Shortcuts*.

## Known limits

- **A virtual cable is required** for system-wide processing; there is no APO,
  Audio Server Plugin or PipeWire filter in this project.
- **AutoEQ is parametric only** — AutoEQ TXT/JSON and OPRA JSON are accepted;
  GraphicEQ text and FIR/convolution profiles are not imported.
- **Spatial audio is a physical model, not a measured HRTF**; externalization
  varies between listeners. Surround is a stereo upmix, not 7.1 decoding.
- **Adaptive EQ is not room or headphone correction.** It balances program
  material across five octave bands with a deterministic controller.
- **Battery and volume sync are Windows features.** Other platforms keep the
  master volume inside the DSP and do not read HID battery reports.
- **Windows is the reference platform.** The macOS and Linux builds use the
  same portable CPAL/Tauri code; packaging and routing steps differ as above.

## Contributing

Issues and pull requests are welcome. Please keep the audio path allocation-free,
run `cargo test --workspace` and `npm run build` before opening a PR, and avoid
adding cloud dependencies to features that can run locally. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the ground rules.

## License

Audio Pulse is free software under the **GNU General Public License v3.0 or
later**. You may use, study, modify and redistribute it; any distributed
derivative must remain open source under the same terms. See [LICENSE](LICENSE).

Third-party components and their licenses are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The bundled headphone profiles
are derived from public measurements (Rtings and oratory1990) converted to the
AutoEQ parametric model; they are not measurements of your unit, and the project
is not affiliated with any headphone manufacturer.

<p align="center">
  Built by <a href="https://github.com/DouglasGDev">Douglas Groff</a> ·
  <a href="https://github.com/DouglasGDev/audio-pulse/issues">Issues</a> ·
  <a href="README.md">Leia em português</a>
</p>
