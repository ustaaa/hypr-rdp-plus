# Notes for agents working on hypr-rdp-plus

## Hardware acceleration first

- Encoding and capture must try the GPU first and use software only as a
  fallback. Keep `--h264-backend auto` (VA-API, then OpenH264) as the default;
  never make a software-only path the default.
- Prefer `--capture-mode ext`: it captures DMA-BUF frames and hands them to
  VA-API without a CPU copy, and falls back to shared memory by itself.
- AVC444 is currently software-only (`SoftwareAvc444`). Do not enable it by
  default until it has a hardware path.
- To check a running server, read `~/.local/state/hypr-rdp-microphone/server.log`:
  look for "Using VA-API hardware encoder" and "zero-copy DMA-BUF".

## Names

The project is called hypr-rdp-plus. The crate, binary (`hypr-rdp`) and the
Omarchy wrapper (`~/.local/bin/hypr-rdp-microphone`) keep their old names so
existing installs and synced configs keep working.

## Checks before committing

- `cargo test --release --locked`
- `python3 -m unittest discover -s integrations/omarchy-remote-desktop/tests`
