# segotep-be-aced

Rust reimplementation of Segotep **BeAced** digital-display control.

- **GUI** (default): borderless 500×450 window laid out like official `LEDDisplay`
  (metric buttons, sensor list, screen toggle; English only) — assets redrawn, not copied
- **CLI** still available for headless use
- Vendor binaries stay under [`official/`](official/) (read-only reference)

## Features

- GUI — official-like control window (opens when run with no args)
- `list` — enumerate HID devices and highlight BeAced IDs
- `info` — flash profile + matched device + live sensor snapshot
- `daemon` — headless poll + HID push
- `screen on|off` — one-shot screen toggle report
- `validate` — dump packets for USB capture comparison

## Protocol summary

BeAced uses the official **screen** path (`official/screen` marker present):

| | |
|---|---|
| VID:PID | `0x0416:0x6001` (Nuvoton) |
| Report | 11 bytes, see [`docs/protocol.md`](docs/protocol.md) |

## Build

```bash
cargo build --release
```

That single command also builds `sensor-bridge` (LibreHardwareMonitor) and copies it
next to `target/release/segotep-be-aced.exe`. Needs **MSVC** + **.NET 8 SDK** on PATH.

Then run:

```bash
.\target\release\segotep-be-aced.exe
```

Approve the UAC prompt (Administrator) — required for CPU temperature/power, same as official.

## Usage

```bash
# GUI (same as double-clicking the release exe)
cargo run --release
# or
cargo run --release -- gui

# optional: write default config next to cwd
cargo run -- init-config

# CLI helpers
cargo run -- list
cargo run -- info
cargo run -- daemon --once
cargo run -- screen off
cargo run -- validate --mode screen
```

Close the official Segotep Digital app before using this tool so HID access is free.

Config file: [`be-aced.toml`](be-aced.toml) (root). Sensor labels / flash profile can
also be read **read-only** from `official/config` and `official/flashValue`.

```bash
RUST_LOG=debug cargo run -- daemon --once
```

## Notes

- Close the official app before `daemon` / `screen` — it may hold the HID device exclusively.
- CPU temperature/load/clock come from `sysinfo`. Package power and rich GPU sensors are limited without LibreHardwareMonitor; values default to `0` when unavailable.
- Run elevated if Windows blocks sensor or HID access on your setup.

## Layout

```text
.
├── official/          # vendor app (do not modify)
├── docs/protocol.md   # HID notes from decompile + capture
├── be-aced.toml       # runtime config
├── Cargo.toml
└── src/
    ├── main.rs
    ├── gui.rs          # official-like window
    ├── config.rs
    ├── device.rs
    ├── protocol.rs
    ├── sensors.rs
    └── daemon.rs
```
