# LAN Pairing Design

This document defines the future LAN pairing direction for Copi native clients.

## Goal

LAN mode should feel like device pairing, not like network configuration.

The user-facing idea is:

1. Open Copi on a trusted device.
2. Tap "Link Device".
3. Show or enter a short pairing code.
4. The new device appears in "My Devices".
5. Clipboard sync starts automatically.

Users should not need to understand tokens, ports, multicast, or peer URLs.

## Product Model

Use these terms in product UI:

- "My Devices": devices linked to the same sync group.
- "Link Device": starts pairing a new device.
- "Pairing Code": a short temporary code shown to the user.
- "Sync Key": internal long-lived secret used after pairing.

Avoid showing "token" in native app UI. Token can remain a CLI/backend term during the MVP, but the product concept is pairing.

Suggested Chinese UI labels:

- "我的设备" for "My Devices".
- "关联设备" for "Link Device".
- "查看匹配码" for "View Pairing Code".
- "匹配码" for "Pairing Code".
- "同步密钥" for the internal sync key when it must be named in developer-facing settings.

## Main Screen Shape

The LAN settings screen should roughly contain:

- Clipboard sync toggles at the top.
- A "My Devices" list with device names and platform icons.
- A per-device action for viewing a pairing code when that device can invite another device.
- A primary "Link Device" button at the bottom.

The important behavior is that pairing is attached to devices. The UI should make it clear which devices are linked and whether a device needs an extra permission step.

## Pairing Flow

### Add A New Device

1. Existing device selects "Link Device".
2. Existing device creates a short-lived pairing session.
3. Existing device displays a pairing code, and later a QR code can be added.
4. New device selects "Join Existing Devices".
5. New device enters or scans the pairing code.
6. Devices exchange enough information to join the same sync group.
7. Both devices persist the sync key locally.
8. The device list updates on both sides.

### View Pairing Code

From "My Devices", a device can expose "View Pairing Code" when it is allowed to invite another device. This should create a new temporary pairing code. It should not show the long-lived sync key.

## Security Rules

- Pairing codes are short-lived and single-purpose.
- Pairing codes are not the long-lived sync key.
- The sync key must not be printed in normal UI, logs, or diagnostics.
- Removing a device should stop that device from receiving future clipboard sync.
- In a stronger later version, removing a device should rotate the sync key for the remaining devices.
- LAN sync should authenticate every clipboard write.
- LAN sync can start with HTTP for MVP, but the protocol should leave room for encrypted peer transport later.

## MVP Mapping

The current CLI already supports:

```bash
copi client --lan --token <shared-secret>
```

For the first native-client MVP, the native shell can:

1. Generate a random sync key.
2. Store it in the platform keychain or secure storage.
3. Pass it to the Go CLI as `--token`.
4. Present only pairing code / linked-device UI to the user.

This keeps the Go core simple while giving the product the right shape.

## Discovery Notes

LAN mode uses IPv4 UDP multicast for automatic peer discovery. Wired and Wi-Fi
devices can discover each other when they are on the same bridged LAN and the
router or access point forwards multicast between Ethernet and Wi-Fi.

Discovery can fail when:

- the Wi-Fi network is a guest network
- AP/client isolation is enabled
- wired and wireless clients are on different VLANs or subnets
- local firewalls block UDP multicast or the peer HTTP port
- the router drops multicast between Ethernet and Wi-Fi

The peer HTTP transport port is not fixed by default. The current core listens
on `0.0.0.0:0`, lets the operating system choose a random available high port,
then announces the actual port to peers. Discovery still uses the shared
multicast address `239.255.27.42:9529` so devices know where to find each other.

The current core announces on every active non-loopback IPv4 multicast-capable
interface when the listen address is a wildcard such as `0.0.0.0:0`. This makes
multi-interface machines and mixed wired/Wi-Fi LANs more reliable, but it cannot
cross networks that intentionally block multicast.

## Future CLI/Core Work

Later LAN pairing can become first-class in the Go core:

- Pairing session creation.
- Pairing-code verification.
- Device metadata exchange.
- Device list persistence.
- Device removal and key rotation.
- Machine-readable pairing events for native shells.

Possible future commands or daemon APIs can be designed after the native shell shape is clearer. Do not add a user-facing `copi config` style command for pairing; pairing should be an app workflow.

## Non-Goals For Now

- Account login.
- Cloud relay-based pairing.
- File transfer.
- Full end-to-end encrypted LAN transport.
- Cross-platform background behavior on mobile.

Those can be added later without changing the user model.
