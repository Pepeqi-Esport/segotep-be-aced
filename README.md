# Segotep BeAced Digital Display Control

Rust reimplementation of Segotep **BeAced** digital-display control.

* **GUI** (default): egui window (~760×760) with live metrics, display toggle, theme, and launch-on-startup — assets redrawn, not copied
* Close minimizes to the **system tray** (display keeps updating); use **Quit completely** to exit
* **CLI** still available for headless use
* Vendor binaries stay under [`official/`](official/) (read-only reference)

## Features

* GUI — control window (opens when run with no args, or `gui`)
* `--tray` — start hidden in the system tray (used by login autostart)
* `list` — enumerate HID devices and highlight BeAced IDs
* `info` — flash profile + matched device + live sensor snapshot
* `daemon` — headless poll + HID push (`--once`,   `--dry-run`,   `--screen-off`,   `--background`)
* `screen on|off` — one-shot screen toggle report
* `validate` — dump packets for USB capture comparison
* `init-config` — write default `config.json` + `theme.json` beside the exe if missing
* In-app updates — startup check + Settings **Check for updates** (GitHub Releases)

## Protocol summary

Two HID paths (auto-detected; override with `--mode` ):

| Mode | VID: PID | Report | Official path |
|------|---------|--------|---------------|
| `screen` (default when `official/screen` marker present) | `0x0416:0x6001` (Nuvoton) | 11 bytes | `SendScreenValues` |
| `legacy` | `0x1A86:0xA001` | 34 bytes | `SendLatestValues` |

Packet builders live in [ `src/services/protocol_service.rs` ](src/services/protocol_service.rs).

## Build

```bash
cargo build --release
```

That command also builds `sensor-bridge` (LibreHardwareMonitor) and embeds it into the exe (extracted beside the binary on first run). Needs **MSVC** (or a Windows Rust toolchain) + **. NET 8 SDK** on PATH for the bridge build.

Then run:

```bash
.\target\release\segotep-be-aced.exe
```

Approve the UAC prompt (Administrator) — required for CPU temperature/power and reliable HID access, same as official. The app also checks for the **. NET 8 Windows Desktop** runtime used by `sensor-bridge` and can offer to install it.

## Usage

```bash
# GUI (same as double-clicking the release exe)
cargo run --release
# or
cargo run --release -- gui

# start hidden in tray (login autostart uses this)
cargo run --release -- --tray

# optional: ensure default config/theme beside the exe
cargo run -- init-config

# CLI helpers
cargo run -- list
cargo run -- info
cargo run -- daemon --once
cargo run -- screen off
cargo run -- validate --mode screen
```

Close the official Segotep Digital app before using this tool so HID access is free.

### Config

On every launch the app ensures these files exist beside the exe (same portable layout as s3-manager):

| File | Purpose |
|------|---------|
| [ `config.json` ](config.json) | Poll interval, flash profile, sensor label mapping |
| [ `theme.json` ](theme.json) | UI theme ( `system` / `light` / `dark` ) |
| `update.json` | Dismissed GitHub release version ( `Later` ) |
| `.env` | Local flags such as `ENABLE_UPDATE_CHECK` |

Legacy [ `be-aced.toml` ](be-aced.toml) is migrated automatically into those JSON files when present.

### Updates

Toggle in-app update checking via `.env` (project root for `cargo run` , or beside the `.exe` when packaged):

```env
ENABLE_UPDATE_CHECK=true
```

Default is `false` . When enabled, the app checks [GitHub Releases](https://github.com/Pepeqi-Esport-Reborn/segotep-be-aced/releases) on startup and via **Check update** in the header.

* **Later** stores that remote version in `update.json` so the same release is not auto-prompted again; a newer release prompts again.
* Manual **Check update** always reports status and can offer Update even for a dismissed version.

Publish a release whose tag matches [ `Cargo.toml` ](Cargo.toml) `version` (e.g. `v0.2.0` ), and attach a Windows zip named for the target triple:

```text
segotep-be-aced-v0.2.0-x86_64-pc-windows-msvc.zip
```

The zip must contain `segotep-be-aced.exe` at the archive root. Bump the crate version before tagging.

Sensor labels / flash profile can also be merged **read-only** from `official/config` and `official/flashValue` when `--official` (default: `official` ) exists.

```bash
RUST_LOG=debug cargo run -- daemon --once
```

## Notes

* Close the official app before `daemon` / `screen` — it may hold the HID device exclusively.
* CPU temperature / load / clock / package power come from `sysinfo` + `sensor-bridge` (LibreHardwareMonitor). Values default to `0` when unavailable.
* Closing the window keeps a background daemon + tray icon; quit from the tray menu or **Quit completely** in Settings.
* Launch on startup registers a scheduled task that starts elevated with `--tray` (no UAC prompt at login).

## Layout

```text
.
├── official/              # vendor app (do not modify)
├── sensor-bridge/         # .NET 8 LibreHardwareMonitor sidecar
├── assets/images/logo/    # app icon + in-window logo
├── assets/fonts/          # Inter (UI typeface) + OFL license
├── config.json            # runtime device/sensor settings
├── theme.json             # UI theme preference
├── update.json            # dismissed update version (created at runtime)
├── be-aced.toml           # legacy config (auto-migrated)
├── build.rs               # embed icon, bridge payload, admin manifest
├── Cargo.toml
└── src/
    ├── main.rs            # CLI + entry
    ├── config/            # AppConfig, flash, sensors, theme
    ├── services/          # HID, protocol, sensors, daemon, background
    ├── helpers/           # admin, tray, autostart, update, payload, storage
    ├── utils/             # constants, theme palette
    └── views/             # egui pages / layouts / components
```
