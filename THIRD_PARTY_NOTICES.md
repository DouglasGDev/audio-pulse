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
| [Depth Anything V2 Small](https://huggingface.co/onnx-community/depth-anything-v2-small) | Apache-2.0 | fp32 ONNX model fetched on first use of the Ear page and verified against a pinned SHA-256. |
| [VB-CABLE Virtual Audio Cable](https://vb-audio.com/Cable/) | Donationware (VB-Audio) | Windows installer fetched from the official vendor URL and run only after the user asks; not bundled. |
| [BlackHole 2ch](https://existential.audio/blackhole/) | GPL-3.0 | macOS installer fetched from the official vendor URL and checksum-verified; not bundled. |

## Reference data and profile formats

| Source | License | Notes |
| --- | --- | --- |
| [AutoEQ](https://github.com/jaakkopasanen/AutoEq) | MIT | Parametric filter model, TXT/JSON profile formats and the conversion method used for the built-in measured corrections. |
| [OPRA](https://github.com/opra-project/OPRA) | Publicly documented dataset | The importer accepts OPRA parametric profiles; no OPRA database is bundled. |
| [Rtings](https://www.rtings.com/headphones) | Measurement data, attributed | Headphone measurements (HMS II.3 and B&K 5128 fixtures) used as the target of some built-in corrections. No Rtings pages or raw data are redistributed. |
| [oratory1990](https://www.reddit.com/r/oratory1990/wiki/index) | Measurement data, attributed | Headphone measurements used as the target of some built-in corrections. No raw data is redistributed. |

The built-in corrections are parametric filters derived from those public
measurements, not measurements of the user's own unit. Audio Pulse is not
affiliated with AutoEQ, OPRA, Rtings or oratory1990.

Audio Pulse is not affiliated with Microsoft, Hugging Face, VB-Audio or the
Depth Anything authors. All trademarks belong to their respective owners.
