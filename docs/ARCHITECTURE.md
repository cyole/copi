# Copi Architecture

Copi is now split into a Go CLI core plus future native clients.

## Product Shape

The user-facing behavior is:

1. Copy on one device.
2. Other devices receive the newest clipboard payload.
3. Paste works locally on those devices.

The Go CLI core is intentionally headless. Native apps should wrap it first, so each platform only needs to build a native shell around one shared sync engine.

## CLI Core Principle

The CLI owns:

- relay-backed client sync
- third-party HTTP relay
- LAN discovery and peer sync
- protocol compatibility
- clipboard polling/apply behavior
- machine-readable capability reporting

Native shells own:

- settings UI
- tray/menu-bar/status UI
- launch-at-login/startup integration
- notifications
- platform-specific packaging and updates

The shell should not reimplement the sync algorithm. It should launch `copi client --relay ...` or `copi client --lan`, pass configuration through flags or environment variables, and use `copi version --json` for feature detection.

The stable shell-facing contract lives in [CLI_CONTRACT.md](CLI_CONTRACT.md).

## Runtime Modes

### Relay-Backed Mode

`copi relay` starts a third-party HTTP relay. This process does not read from or write to its own clipboard. It can run on a cloud VM, NAS, mini PC, or any always-on machine.

`copi client --relay ...` runs on each desktop device, watches the local clipboard, publishes changes, long-polls the relay, and applies remote changes to the local clipboard.

### LAN Mode

`copi client --lan` starts a local HTTP peer endpoint and UDP multicast discovery. Peers announce their HTTP URL every few seconds. When the local clipboard changes, the device posts the new payload to every known peer.

The future native-client UX for LAN mode should use device pairing and short-lived pairing codes instead of asking users to type tokens. The detailed product and implementation direction lives in [LAN_PAIRING.md](LAN_PAIRING.md).

## Protocol

Clipboard payloads are wrapped in an envelope:

```json
{
  "id": "dev-1-1779273600000000000",
  "device_id": "dev-1",
  "device_name": "MacBook",
  "seq": 1,
  "timestamp": "2026-05-20T00:00:00Z",
  "payload": {
    "type": "text",
    "mime": "text/plain; charset=utf-8",
    "text": "hello"
  },
  "hash": "..."
}
```

Current payload support is text. The payload model keeps `mime` and `data` fields so image, file, and rich text support can be added without replacing the transport.

## Native Client Plan

Native clients should keep platform UI and clipboard code native while reusing the same protocol.

- macOS: SwiftUI app with menu bar mode, NSPasteboard clipboard integration, launch-at-login support.
- Windows: WinUI 3 + C#/.NET app with system tray, Windows Clipboard API integration, startup task support.
- Linux: GTK/libadwaita or Qt app with Wayland/X11 clipboard support and desktop autostart.
- iOS/iPadOS: SwiftUI app with UIPasteboard. Background clipboard behavior needs platform-specific review.
- Android: Kotlin + Jetpack Compose with ClipboardManager. Background clipboard access depends on Android version restrictions.

The first practical client milestone is a native settings shell that can choose relay-backed mode or LAN mode, manage paired LAN devices, configure the relay URL, and manage startup behavior while delegating sync to the Go daemon.
