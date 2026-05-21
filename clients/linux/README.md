# Copi Linux Client

This is the first Linux native shell for Copi. It is a Go GTK tray app that launches the Go CLI core as a child process.

## Dependencies

Ubuntu/Debian:

```bash
sudo apt install libgtk-3-dev pkg-config
```

Fedora:

```bash
sudo dnf install gtk3-devel pkgconf-pkg-config
```

The source build uses `gotk3`, so GTK 3 development files are required at build time. Runtime packages need `libgtk-3-0` or the distribution equivalent.

## Run From Source

Run this on a Linux machine:

```bash
./clients/linux/run.sh
```

The script builds:

```bash
go build -o clients/linux/build/copi ./cmd/copi
go build -tags gtk_3_20 -o clients/linux/build/copi-linux-gui ./clients/linux/cmd/copi-linux-gui
```

Then it starts the GTK shell with `COPI_CLI` pointing at that CLI binary.
The GUI build targets gotk3's GTK 3.20 bindings to avoid a broken GTK 3.22 GDK wrapper in gotk3 v0.6.4.

To use an existing CLI binary:

```bash
COPI_CLI=/path/to/copi ./clients/linux/run.sh
```

## Build Packages

Run this on a Linux machine:

```bash
./clients/linux/package.sh
```

It writes:

```text
clients/linux/dist/copi-linux-<version>-<arch>.tar.gz
clients/linux/dist/copi-linux-<version>-<arch>.deb
```

The `.deb` is created when `dpkg-deb` is available.

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

The CLI path is intentionally not shown in the user interface. The app resolves it internally from `COPI_CLI`, a sibling `copi`, `/usr/bin/copi`, or `PATH`.
