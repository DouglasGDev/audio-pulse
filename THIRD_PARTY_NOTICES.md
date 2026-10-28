# Third-party notices

Audio Pulse is free software under the GNU General Public License v3.0 or
later. It builds on the components below.

## Linked into the application

| Component | License | Notes |
| --- | --- | --- |
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) | MIT | Statically linked through the `ort` crate; runs the local depth model. |
| [Tauri](https://tauri.app/) | Apache-2.0 / MIT | Desktop shell, IPC, tray and updater primitives. |
| [React](https://react.dev/) / [Zustand](https://zustand.docs.pmnd.rs/) | MIT | Interface and state store. |
| Rust crates | permissive (MIT / Apache-2.0 / BSD) | Full resolution in `Cargo.lock`. |
| npm packages | MIT / ISC / Apache-2.0 | Full resolution in `package-lock.json`. |

## Downloaded at runtime, never redistributed here

| Component | License | Notes |
| --- | --- | --- |
| [Depth Anything V2 Small](https://huggingface.co/onnx-community/depth-anything-v2-small) | Apache-2.0 | int8 ONNX model fetched on first use of the Ear page and verified against a pinned SHA-256. |
| [VB-CABLE Virtual Audio Cable](https://vb-audio.com/Cable/) | Donationware (VB-Audio) | Windows installer fetched from the official vendor URL and run only after the user asks; not bundled. |
| [BlackHole 2ch](https://existential.audio/blackhole/) | GPL-3.0 | macOS installer fetched from the official vendor URL and checksum-verified; not bundled. |

Audio Pulse is not affiliated with Microsoft, Hugging Face, VB-Audio or the
Depth Anything authors. All trademarks belong to their respective owners.
