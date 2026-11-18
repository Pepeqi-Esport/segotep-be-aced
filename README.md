# Segotep BeAced

Rust reimplementation of Segotep **BeAced** digital-display control.

- **GUI** (default): egui window (~760×760) with live metrics, display toggle, theme, and launch-on-startup — assets redrawn, not copied
- Close minimizes to the **system tray** (display keeps updating); use **Quit completely** to exit
- **CLI** still available for headless use
- Vendor binaries stay under [`official/`](official/) (read-only reference)

## Features

- GUI — control window (opens when run with no args, or `gui`)
- `--tray` — start hidden in the system tray (used by login autostart)
- `list` — enumerate HID devices and highlight BeAced IDs
- `info` — flash profile + matched device + live sensor snapshot
- `daemon` — headless poll + HID push (`--once`, `--dry-run`, `--screen-off`, `--background`)
- `screen on|off` — one-shot screen toggle report
- `validate` — dump packets for USB capture comparison
- `init-config` — write default `config.json` beside the exe if missing

## Protocol summary

Two HID paths (auto-detected; override with `--mode`):

| Mode                                                     | VID: PID                  | Report   | Official path      |
| -------------------------------------------------------- | ------------------------- | -------- | ------------------ |
| `screen` (default when `official/screen` marker present) | `0x0416:0x6001` (Nuvoton) | 11 bytes | `SendScreenValues` |
| `legacy`                                                 | `0x1A86:0xA001`           | 34 bytes | `SendLatestValues` |

Packet builders live in [`src/services/protocol_service.rs`](src/services/protocol_service.rs).

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

# optional: ensure default config beside the exe
cargo run --release -- init-config

# CLI helpers
cargo run --release -- list
cargo run --release -- info
cargo run --release -- daemon --once
cargo run --release -- screen off
cargo run --release -- validate --mode screen
```

Close the official Segotep Digital app before using this tool so HID access is free.

### Config

Runtime file beside the exe (created on first run):

| File                         | Purpose                                                                             |
| ---------------------------- | ----------------------------------------------------------------------------------- |
| [`config.json`](config.json) | Poll interval, UI theme (`system` / `light` / `dark`), flash profile, sensor labels |

Legacy [`be-aced.toml`](be-aced.toml) and older `theme.json` are migrated automatically into `config.json` when present.

Sensor labels / flash profile can also be merged **read-only** from `official/config` and `official/flashValue` when `--official` (default: `official`) exists.

```bash
RUST_LOG=debug cargo run --release -- daemon --once
```

## Notes

- Close the official app before `daemon` / `screen` — it may hold the HID device exclusively.
- CPU temperature / load / clock / package power come from `sysinfo` + `sensor-bridge` (LibreHardwareMonitor). Values default to `0` when unavailable.
- Closing the window keeps a background daemon + tray icon; quit from the tray menu or **Quit completely** in Settings.
- Launch on startup registers a scheduled task that starts elevated with `--tray` (no UAC prompt at login).
- UI typeface is **Inter** (`assets/fonts/`).

## Layout

```text
.
├── official/              # vendor app (do not modify)
├── sensor-bridge/         # .NET 8 LibreHardwareMonitor sidecar
├── assets/images/logo/    # app icon + in-window logo
├── assets/fonts/          # Inter (UI typeface) + OFL license
├── config.json            # runtime device/sensor/theme settings
├── be-aced.toml           # legacy config (auto-migrated)
├── build.rs               # embed icon, bridge payload, admin manifest
├── Cargo.toml
└── src/
    ├── main.rs            # CLI + entry
    ├── config/            # AppConfig, flash, sensors, theme
    ├── services/          # HID, protocol, sensors, daemon, background
    ├── helpers/           # admin, tray, autostart, payload, storage
    ├── utils/             # constants, fonts, theme palette
    └── views/             # egui pages / layouts / components
```
