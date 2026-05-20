# Copi macOS Client

This is the first native shell for Copi. The app is a SwiftUI menu bar app that launches the Go CLI core as a child process.

## Open In Xcode

```bash
open clients/macos/Copi.xcodeproj
```

Choose the `Copi` scheme and press Run. Copi appears in the macOS menu bar instead of opening a main window.

The Xcode target has a build phase named `Build Go CLI`. It runs:

```bash
go build -o <Copi.app>/Contents/Resources/copi ./cmd/copi
```

The app then uses the bundled CLI by default.

If Xcode command-line tools are not selected, switch to the full Xcode developer directory:

```bash
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
```

## Run From Terminal

```bash
./script/build_and_run.sh
```

Useful modes:

```bash
./script/build_and_run.sh --verify
./script/build_and_run.sh --logs
./script/build_and_run.sh --debug
```

## Current MVP

- choose relay mode or LAN mode
- configure relay URL, token, device name, and device ID
- generate a LAN sync key
- start and stop `copi client`
- show process status from the menu bar
- open a dedicated log window from the menu bar
- enable launch at login from Settings, default off

All settings live in the app's Settings window, opened from the menu bar item.

LAN pairing UI is still a product direction document for now. See [../../docs/LAN_PAIRING.md](../../docs/LAN_PAIRING.md).
