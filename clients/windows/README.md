# Copi Windows Client

This is the first Windows native shell for Copi. It is a WinUI 3 tray app that launches the Go CLI core as a child process.

## Dependencies

- Windows 10 2004 or newer
- Go
- .NET 8 SDK
- Windows App SDK workload support for WinUI 3

## Run From Source

Run this in PowerShell on Windows:

```powershell
.\clients\windows\run.ps1
```

The script builds:

```powershell
go build -o clients\windows\build\copi.exe .\cmd\copi
dotnet run --project clients\windows\Copi.Windows\Copi.Windows.csproj
```

Then it starts the WinUI shell with `COPI_CLI` pointing at that CLI binary.

## Build Package

Run this in PowerShell on Windows:

```powershell
.\clients\windows\package.ps1 -Version dev -Runtime win-x64
```

It writes:

```text
clients\windows\dist\copi-windows-<version>-win-x64.zip
```

WinUI 3 apps are distributed as a folder-style app for this MVP, so the package is a zip containing the published app plus `copi.exe`.

## Current MVP

- tray cloud icon with start/stop, settings, logs, and quit actions
- WinUI 3 settings window
- relay mode and LAN mode settings
- relay URL and access token fields
- LAN sync key generation
- local device name and device ID generation
- child-process supervision for `copi client`
- JSON log display in a dedicated log window
- optional startup from Settings, default off, via the current user's Windows Run registry key

Settings are stored at:

```text
%AppData%\Copi\windows-gui.json
```

The sync core path is intentionally not shown in the user interface. The app resolves it internally from `COPI_CLI`, `COPI_CLI_PATH`, a sibling `copi.exe`, `%ProgramFiles%\Copi\copi.exe`, or `PATH`.
