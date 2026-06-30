# Cornefig

Firmware for my **wireless split Corne** (foostan 6-column) on **nice!nano** boards (nRF52840, Adafruit UF2 bootloader). Matrix pinout matches the ZMK Corne shield + nice!nano `pro_micro` pin map.

Features:
- **SSD1306 OLED** on both halves (128×32, I2C via TWISPI0, P0.17/P0.20)
- **Vial** support for live keymap editing
- **4 layers** matching the default/lower/raise layout
- Mod-tap on top-left (Tab tap / Escape hold)

## Roles

| Half   | Flash this binary | Notes                                      |
|--------|-------------------|--------------------------------------------|
| **Left**  | `central`   | USB to the host; BLE to the computer       |
| **Right** | `peripheral`| Talks to the left half over BLE split only |

## Prerequisites

- Rust toolchain (edition **2024** requires **1.85+**): [rustup](https://rustup.rs)
- Cortex-M4F target: `rustup target add thumbv7em-none-eabihf`
- LLVM tools: `rustup component add llvm-tools`
- Build tools: `cargo install cargo-make flip-link cargo-binutils cargo-hex-to-uf2`

### NixOS

On NixOS, use the flake to get `clang` (needed by bindgen for Nordic SDC bindings):

```bash
nix develop
cargo make uf2 --release
```

## Build firmware

```bash
cargo make uf2 --release
```

Artifacts:
- `corne-rmk-central.uf2` → **left** half (central)
- `corne-rmk-peripheral.uf2` → **right** half (peripheral)

## Flash

1. Double-tap reset on the nice!nano → mounts as `NICENANO`
2. Copy the UF2: `cp corne-rmk-central.uf2 /run/media/$USER/NICENANO/`
3. Repeat for the other half with `corne-rmk-peripheral.uf2`

## Keymap

| Layer   | Content |
|---------|---------|
| **0** (default) | QWERTY with `TH(Tab, Escape)`, LShift/LCtrl on bottom row, thumb cluster: MO(1) → LGui → Space / Enter → MO(2) → RAlt |
| **1** (lower)   | Number row, arrows on right hand, rest transparent |
| **2** (raise)   | F1–F10, symbols on right half |
| **3** (empty)   | No default bindings — configure via Vial |

## Display

OLED configured in `keyboard.toml` under `[split.central.display]` and `[split.peripheral.display]` (SSD1306, 128×32, I2C 0x3C).

## Configuration

- **`keyboard.toml`** — matrix pins, split geometry, display, BLE, keymap, Vial unlock
- **`vial.json`** — Vial layout and custom keycodes (Bluetooth actions)
- **`Cargo.toml`** — RMK revision pin and feature flags
- **`memory.x`** — Flash/RAM layout for Adafruit nRF52 UF2 bootloader

## Bluetooth

- **BT0–BT2** switch BLE host slot on key release
- **Clear Peer** — hold ~5 seconds to clear bonding
- Default: 3 BLE slots (set via `ble_profiles_num` in `keyboard.toml`)

## Battery

Both halves use `battery_adc_pin = "vddh"` for chip supply voltage reporting over BLE.

## Nix flake

`flake.nix` provides `clang` and patched glibc headers so `nix develop` gives a working build environment.

## Author

**DFLTPLYR** — gonzales.johncris01@gmail.com
