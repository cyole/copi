# Copi Linux Client

This is the first Linux native shell for Copi. It is a GTK tray app that launches the Go CLI core as a child process.

## Dependencies

Ubuntu/Debian:

```bash
sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1
```

Fedora:

```bash
sudo dnf install python3-gobject gtk3 libayatana-appindicator-gtk3
```

If AppIndicator is not available, the app falls back to `Gtk.StatusIcon`. On modern GNOME, AppIndicator support may require the distribution's tray indicator extension.

## Run From Source

Run this on a Linux machine:

```bash
./clients/linux/run.sh
```

The script builds:

```bash
go build -o clients/linux/build/copi ./cmd/copi
```

Then it starts the GTK shell with `COPI_CLI` pointing at that binary.

To use an existing CLI binary:

```bash
COPI_CLI=/path/to/copi ./clients/linux/run.sh --no-build
```

## Current MVP

- tray cloud icon with start/stop, settings, logs, and quit actions
- relay mode and LAN mode settings
- relay URL and access token fields
- LAN sync key generation
- local device name and device ID generation
- child-process supervision for `copi client`
- JSON log display in a dedicated log window
- optional autostart from Settings, default off, via `~/.config/autostart/com.cyole.copi.desktop`

Settings are stored at:

```text
~/.config/copi/linux-gui.json
```

The CLI path is intentionally not shown in the user interface. The app resolves it internally from `COPI_CLI`, `clients/linux/build/copi`, a sibling `copi`, or `PATH`.
