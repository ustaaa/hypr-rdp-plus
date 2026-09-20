# Remote microphone integration

This build adds opt-in client-to-host microphone redirection to Hypr-RDP.
Enable `--microphone` (or `microphone = true` in TOML). Desktop sound output
(`audio_mode`) is independent. The remote client must allow microphone
redirection and have microphone permission in its OS.

The source appears as **Hypr-RDP Remote Microphone** after the client agrees to
capture. Select it in the receiving application. It disappears on disconnect.
PCM S16LE at 44.1 or 48 kHz, mono/stereo is supported. A bounded 100 ms buffer
keeps newest audio on overflow and emits silence on underrun. Raw audio is not
logged. No speaker autoconnection or system default microphone change occurs.

## Source and dependencies

`../hypr-rdp-ironrdp` retains original IronRDP revision `5198cde` and adds
upstream PR #1945, #1946 and its state-race fixes. #1946 remains an unmerged
upstream change; this is a locally maintained build, not an official release.

- https://github.com/Devolutions/IronRDP/pull/1945
- https://github.com/Devolutions/IronRDP/pull/1946

First prepare the patched dependency with `scripts/prepare-ironrdp.sh`.
Build: `cargo build --release --locked`
Test: `cargo test --locked --bin hypr-rdp`

`tests/manual/microphone-loopback.py` sends a generated 440 Hz tone through a
local FreeRDP client into the RDP server, records only the synthetic source,
and checks sample count, amplitude and frequency correlation. It also removes
its temporary audio sink and closes the client on exit. This validates the
local protocol/PipeWire path, not Android/macOS/Windows interoperability.

## Installation and controls

Build using [BUILD-MICROPHONE.md](BUILD-MICROPHONE.md). The binary can run directly
with `--microphone=true`; use `--microphone=false` to disable input. Keep your
existing bind, credentials, certificate and monitor configuration. Treat a
service restart as disruptive: it disconnects an active RDP client.

For optional Omarchy UI controls, follow the bundled
[integration installation guide](integrations/omarchy-remote-desktop/INSTALL-MICROPHONE.md).
It provides an independent microphone toggle, FPS limit and frame queue setting.
The upstream plugin and packaged Hypr-RDP do not automatically gain this feature.

## Validation and client support

- Unit/regression tests, including PCM derived-field compatibility.
- Local FreeRDP synthetic tone capture at 48 kHz mono and a reconnect at
  44.1 kHz stereo; sample counts and frequency correlation checked.
- Microphone source cleanup on disconnect and temporary test sink cleanup.
- Samsung Android/DeX with Windows App: live microphone packets and signal level
  observed; the user confirmed voice input works.
- macOS and Windows physical microphone interoperability remains unverified for
  this microphone build. Upstream plugin compatibility reports are separate.

Android Windows App: edit saved PC > Device & audio redirection > Microphone on;
also grant Android microphone permission. Reconnect and choose **Hypr-RDP Remote
Microphone** in the receiving application. A source is created only after
successful client microphone negotiation. The plugin's **Share audio** switch
controls outgoing sound only.

To roll back, stop the custom server and restore your previous launcher/service
configuration or backed-up plugin directory. This build does not require replacing
the distribution's `/usr/bin/hypr-rdp`. Save backups before changing a working
installation; do not expose an RDP listener publicly as part of testing.
