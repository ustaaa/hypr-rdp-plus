# Resize the existing desktop to the RDP client

This community build offers an opt-in mode for an explicitly selected output:

```text
--output Virtual-1 --resize-output=true
```

Or in the server TOML configuration:

```toml
output = "Virtual-1"
resize_output = true
```

Do not combine this with a fixed `resolution`. The default is false. The
Omarchy integration exposes the setting as a checkbox for the selected monitor.

The server changes the mode of the **existing output**, keeping the current
Hyprland desktop and applications. It does not create an extra screen. Initial
client dimensions and supported DisplayControl layout requests drive changes;
the client must send resize requests for updates during a connection.

The server saves the original monitor geometry for the session, retains its
position, scale and refresh rate when applying a new size, and restores the
saved geometry when that client disconnects or the server shuts down normally.
A failed mode change is rolled back. Requests are subject to compositor and
encoder limits. Multi-monitor layouts and quarter-turn rotated outputs are
not supported by this mode; they retain their current geometry. A compositor
that rejects the requested size also keeps or restores the previous mode.

This is a real monitor mode change, visible to local users and other screen
sharing applications such as Sunshine. Applications may rearrange their
windows when the work area changes; restoring the monitor dimensions does not
promise to restore every application's previous window geometry.

SIGKILL or a machine/process crash cannot run normal cleanup. Keep another
access method available when first testing on a physical monitor. Changes are
runtime changes, not edits to the user's Hyprland monitor configuration file.

## Validation

Local FreeRDP tests on an existing QEMU output changed the same desktop from
2560x1080 at 60 Hz to 1920x1080 and then, in a second session, 1280x720.
Both disconnects restored 2560x1080 at 60 Hz, retaining position, scale and
transform. A third ClearCodec session passed the synthetic remote microphone
test and restored the desktop on disconnect. These tests do not establish
which dynamic resize requests a particular Android/macOS/Windows client sends.

Regression tests cover transactional apply/rollback, restore retry, initial
presentation fallback, canceled resize work, and stale DisplayControl jobs.
Initial requested dimensions are bounded to 3840x2160 during negotiation;
existing capture/encoder limits apply to subsequent layouts too.
