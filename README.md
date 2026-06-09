<div align="center">

# A Rust FC/NES emulator

![Rust](https://img.shields.io/badge/Rust-2024-b7410e?style=for-the-badge&logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-stage%201%20WIP-f4b400?style=for-the-badge)
![Target](https://img.shields.io/badge/target-Super%20Mario%20Bros.-e52521?style=for-the-badge)
![Mapper](https://img.shields.io/badge/mapper-0%20%2F%20NROM-3366cc?style=for-the-badge)
![Tests](https://img.shields.io/badge/tests-74%20passing-2ea44f?style=for-the-badge)

[English](README.md) | [中文](README.zh-CN.md)

</div>

`fc-emu` is a Rust-based FC/NES emulator project. The first development stage focuses on one practical target: loading the local `Super Mario Bros. (Japan, USA).nes` ROM, opening a desktop window, and growing the emulator until the game can be played end to end with video, keyboard input, audio, pause, and save-state support.

The project is intentionally scoped. It does not try to be a full, highly compatible NES emulator in the first stage. Instead, it builds the core pieces around the target ROM first, keeps module boundaries clear, and leaves room to expand mapper support, rendering accuracy, audio accuracy, debugging tools, and compatibility later.

## Project Snapshot

| Area | Choice |
| --- | --- |
| Language | Rust 2024 |
| First target | `Super Mario Bros. (Japan, USA).nes` |
| Cartridge scope | Mapper 0 / NROM first |
| Window stack | `winit` + `pixels` |
| Audio stack | `cpal` |
| Testing | Rust unit tests |

## Current Status

The codebase already contains the first pass of the emulator architecture:

- ROM loading and iNES parsing.
- Mapper 0 / NROM cartridge mapping.
- CPU, PPU, APU, Bus, Input, Save State, Window, and App modules.
- A `winit` + `pixels` desktop window path.
- A `cpal` audio output path with basic pulse, triangle, and noise mixing.
- Keyboard mapping for NES controls and emulator controls.
- Disk-backed save states in three slots under `saves/`.
- Unit tests for parser behavior, mapper behavior, CPU behavior, bus routing, PPU rendering basics, controller reads, save-state persistence, and app/window wiring.

This is still a work in progress. The current stage is for development and validation, not a polished general-purpose emulator release.

## Goal

The overall goal is to build a playable, maintainable FC/NES emulator in Rust. The near-term target is to make `Super Mario Bros.` run smoothly on the developer machine while keeping the core emulator logic testable and separated from platform-specific window, input, and audio code.

After the first target ROM works reliably, the project can expand toward broader ROM compatibility, more mappers, stronger timing accuracy, debugging tools, performance measurement, and a more complete user experience.

## Stage 1 Scope

- [x] Initialize the Rust project and module structure.
- [x] Support a default ROM path and explicit ROM path from the command line.
- [x] Parse iNES ROM headers and extract PRG/CHR data.
- [x] Implement Mapper 0 / NROM behavior.
- [x] Add Bus routing for CPU RAM, PPU registers, APU, controller input, DMA, and cartridge access.
- [x] Add keyboard-to-controller input mapping.
- [x] Add emulator app wiring, pause controls, save/load actions, and window title updates.
- [x] Add initial save-state data structures and validation.
- [x] Add a desktop window path using `winit` and `pixels`.
- [x] Add a basic audio output path using `cpal`.
- [x] Persist save states to disk across sessions.
- [ ] Complete target-ROM CPU behavior and interrupt accuracy.
- [ ] Complete PPU rendering behavior needed for correct gameplay.
- [ ] Complete APU sound behavior and stable synchronization.
- [ ] Finish pause/menu interactions and full gameplay validation.
- [ ] Verify that the target ROM can be played end to end.

## Requirements

- Rust toolchain with Cargo.
- A local `.nes` ROM file for development/testing.

ROM files are not included in this repository. Place your legally obtained ROM at:

```bash
rom/Super Mario Bros. (Japan, USA).nes
```

Or pass a ROM path explicitly when running the project.

## Running

Run with the default ROM path:

```bash
cargo run
```

Run with an explicit ROM path:

```bash
cargo run -- path/to/game.nes
```

## Controls

| NES Button | Keyboard |
| --- | --- |
| Up | Arrow Up |
| Down | Arrow Down |
| Left | Arrow Left |
| Right | Arrow Right |
| A | X |
| B | Z |
| Start | Enter |
| Select | Right Shift, S, or Tab |
| Pause / Resume | Space or P |
| Save State | F5 |
| Load State | F9 |
| Select Slot | 1, 2, 3 |
| Exit | Escape |

Save states are written to the `saves/` directory and are keyed by ROM file name and slot number.

## Known Issues

- Audio is still incorrect; the current APU does not yet reproduce the original `Super Mario Bros.` music.
- After a mushroom is bumped out, the video can still freeze once the mushroom fully appears. This needs further PPU/CPU timing work.

## Development

Run the test suite:

```bash
cargo test
```

The current suite covers the most important low-level behavior that has been implemented so far.

## Architecture

The project is organized around separate emulator modules:

```text
src/
  main.rs
  app.rs
  emulator.rs
  rom.rs
  bus.rs
  input.rs
  save_state.rs
  window.rs
  cpu/
  ppu/
  apu/
  mapper/
```

The core emulator modules are kept separate from the outer application and window layer so they can be tested independently.
