# Optional Omarchy microphone integration

This directory contains the upstream MIT-licensed Omarchy Remote Desktop plugin
with community microphone, FPS and frame queue controls. It uses the same plugin
ID, `kokd.remote-desktop`, and replaces that plugin rather than installing beside
it. Its original `LICENSE` and authorship are retained. Upstream source:
<https://github.com/kostadinkadiev/omarchy-remote-desktop>.

Build the custom backend using [the build guide](../../BUILD-MICROPHONE.md) first.
The original upstream installer in this snapshot is not the custom backend
installer. The integration requires `~/.local/bin/hypr-rdp-microphone`.

## Install the backend and portable wrapper

From the root of the Hypr-RDP microphone repository:

```bash
install -Dm755 target/release/hypr-rdp \
  "$HOME/.local/libexec/hypr-rdp-microphone"
mkdir -p "$HOME/.local/bin"
cat > "$HOME/.local/bin/hypr-rdp-microphone" <<'WRAPPER'
#!/bin/sh
set -eu
umask 077
binary="$HOME/.local/libexec/hypr-rdp-microphone"
# Plugin compatibility checks must receive stdout and the original exit status.
for arg in "$@"; do
    case "$arg" in
        --help|-h|--version|-V) exec "$binary" "$@" ;;
    esac
done
log_dir="${XDG_STATE_HOME:-$HOME/.local/state}/hypr-rdp-microphone"
mkdir -p "$log_dir"
chmod 700 "$log_dir"
log_file="$log_dir/server.log"
touch "$log_file"
chmod 600 "$log_file"
export RUST_LOG="${RUST_LOG:-warn}"
# Forward exactly what the plugin supplies; never append --microphone here.
exec "$binary" "$@" >> "$log_file" 2>&1
WRAPPER
chmod 755 "$HOME/.local/bin/hypr-rdp-microphone"
"$HOME/.local/bin/hypr-rdp-microphone" --version
"$HOME/.local/bin/hypr-rdp-microphone" --help
```

Logs are owner-private. The wrapper does not log arguments or force microphone
activation. The plugin passes `--microphone=true` or `--microphone=false`, so its
toggle remains authoritative. Stop the service before archiving or truncating
large logs; rotation is not installed by these commands.

## Install the plugin snapshot

Use a local desktop or another access method while replacing a working plugin;
disabling/restarting sharing disconnects the active RDP session. Stop sharing and
disable the existing Remote Desktop plugin through Omarchy's plugin controls.
Back up its directory before copying this snapshot:

```bash
plugin_dir="$HOME/.config/omarchy/plugins/kokd.remote-desktop"
backup_root="${XDG_STATE_HOME:-$HOME/.local/state}/hypr-rdp-microphone/backups"
mkdir -p "$backup_root"
chmod 700 "$backup_root"
backup_dir="$backup_root/remote-desktop-$(date +%Y%m%d-%H%M%S)"
if [ -e "$plugin_dir" ]; then
    mv -- "$plugin_dir" "$backup_dir"
fi
mkdir -p "$plugin_dir"
cp -a integrations/omarchy-remote-desktop/. "$plugin_dir/"
python3 "$plugin_dir/bin/remote-desktopctl" check
```

Enable Remote Desktop through Omarchy's plugin controls, open its settings and
choose the private address, monitor and credentials. Settings now include
**Remote microphone**, **Maximum FPS** (15–60) and **Queued frames** (1–3).
Defaults are microphone enabled, 60 FPS and two queued frames. Existing plugin
configurations inherit these defaults, so review them before starting sharing.
Saving settings restarts active sharing. Do not enable a second RDP server on
the same address and port.

Enable microphone forwarding and OS microphone permission in the client too.
Select **Hypr-RDP Remote Microphone** in the receiving application. **Share audio**
controls playback in the other direction and is independent.

The plugin's ID/version retain upstream identity for compatibility; this is a
modified snapshot, not an official upstream release. Updating it from upstream
can overwrite these changes. Its screenshots and general upstream README are
reference material, not evidence of microphone support on every listed client.
See [tested clients](../../MICROPHONE.md#validation-and-client-support).

To roll back, stop sharing and disable this plugin, then restore the backed-up
plugin directory and the previous backend configuration before enabling it.
Keep the custom binary until you confirm no service or launcher still uses it.
