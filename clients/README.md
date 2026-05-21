# Native Clients

Copi's Go CLI is the shared sync engine. Native clients should be thin shells around it: UI, tray/menu-bar behavior, settings, startup integration, notifications, and platform packaging stay native; sync logic stays in the CLI.

The native shell should launch and supervise the CLI instead of reimplementing sync.

## Shell Contract

Native apps should treat the CLI as the product core:

- run `copi version --json` to detect version, commands, and capabilities
- run `copi client --relay <url> --token <token>` for relay-backed sync
- run `copi client --lan --token <token>` for LAN sync
- run `copi relay` only on third-party relay machines, usually through Docker
- store user settings in the native app, then pass them to the CLI as flags or environment variables
- show native status UI by supervising the child process and calling health/status commands

Do not parse human log lines as an API. Anything the shell needs should become a stable JSON command in the CLI.

The current shell contract is documented in [../docs/CLI_CONTRACT.md](../docs/CLI_CONTRACT.md).

## Recommended Stacks

- macOS: SwiftUI + NSPasteboard
- Windows: WinUI 3 + C#/.NET
- Linux: GTK/libadwaita or Qt with Wayland/X11 clipboard support
- iOS/iPadOS: SwiftUI + UIPasteboard
- Android: Kotlin + Jetpack Compose + ClipboardManager

## Windows Choice

For Windows, use WinUI 3 + C#/.NET. It is the product direction for the native Windows shell: settings pages, notifications, tray/background behavior, startup integration, and access to Windows platform APIs.

## First Native Client Milestone

Each native client should start as a small settings shell:

- choose relay-backed mode or LAN mode
- configure the third-party relay URL and token for relay-backed mode
- show local device name and device ID
- start/stop the Go sync daemon
- enable launch at login/startup
- show current connection and peer status

The first implementation should run the Go CLI as a child process. Later, if a platform truly needs deeper integration, it can embed the Go core or reimplement the protocol, but that should not be the default path.

## Current macOS Client

The first native app lives at [macos/Copi.xcodeproj](macos/Copi.xcodeproj).

Open it with:

```bash
open clients/macos/Copi.xcodeproj
```

The target is named `Copi` and bundles the Go CLI during the Xcode build.

## Current Windows Client

The first Windows shell lives at [windows](windows). It is a WinUI 3 tray app with a cloud tray icon and settings/logs windows.

Run it on Windows with:

```powershell
.\clients\windows\run.ps1
```

The script builds `clients\windows\build\copi.exe`, then launches the WinUI shell with that sync core.

## Current Linux Client

The first Linux shell lives at [linux](linux). It is a Go GTK tray app with a cloud tray icon and settings/logs windows.

Run it on Linux with:

```bash
./clients/linux/run.sh
```

The script builds `clients/linux/build/copi` and `clients/linux/build/copi-linux-gui`, then launches the GTK shell with that CLI binary.
