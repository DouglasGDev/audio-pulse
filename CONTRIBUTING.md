# Contributing to Audio Pulse

Thanks for taking the time to improve Audio Pulse. The project is GPL-3.0-or-later:
by submitting a contribution you agree it can be distributed under the same
license.

## Ground rules

1. **Keep audio local-first.** Features must work offline. Cloud APIs, accounts
   and telemetry are out of scope.
2. **No fake processing.** Do not label heuristics as AI, and do not add
   hardcoded fallbacks that pretend to analyze audio or images. If a model or
   measurement is involved, it must actually run.
3. **The audio thread never allocates.** New stages must pre-allocate buffers
   and apply parameter changes at buffer boundaries. Discrete changes go through
   the existing 2 ms down / 30 ms up ramp to avoid clicks.
4. **No new latency in the direct path.** Analysis stages (FFT, envelope
   followers) run beside the audio, not in front of it.
5. **Respect privacy.** Ear photos and audio stay on the device. Any network
   download must be explicit, pinned by checksum and documented.

## Before opening a pull request

```bash
cargo test --workspace
npm run build
```

For interface changes, run the real app (`npm run tauri dev`) and describe what
you tested. For DSP changes, add or update a test that proves the behavior, not
just that the output is finite.

## Style

- Rust: `cargo fmt` and `cargo clippy --workspace --all-targets`.
- TypeScript: strict mode; no `any` unless unavoidable and explained.
- UI copy is bilingual (pt-BR and English) and lives in `src/lib/i18n.ts`.
- Keep comments for non-obvious constraints only.

## Reporting bugs

Include the platform, the audio devices in use (input and output), the buffer
size, whether the route or a preview was running, and the exact steps. For
sound issues, say whether Bypass changes the problem — it separates the DSP from
the routing/driver layer.

## Security

Please do not open a public issue for a vulnerability that could affect users'
machines. Use GitHub's private vulnerability reporting on the repository
instead.
