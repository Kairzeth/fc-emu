# fc-emu

English | [中文](README.zh-CN.md)

`fc-emu` is a Rust-based FC/NES emulator project. The first development stage focuses on one practical target: loading the local `Super Mario Bros. (Japan, USA).nes` ROM, opening a desktop window, and growing the emulator until the game can be played end to end with video, keyboard input, audio, pause, and save-state support.

The project is intentionally scoped. It does not try to be a full, highly compatible NES emulator in the first stage. Instead, it builds the core pieces around the target ROM first, keeps module boundaries clear, and leaves room to expand mapper support, rendering accuracy, audio accuracy, debugging tools, and compatibility later.

## Current Status

The codebase already contains the first pass of the emulator architecture:

- ROM loading and iNES parsing.
- Mapper 0 / NROM cartridge mapping.
- CPU, PPU, APU, Bus, Input, Save State, Window, and App modules.
- A `winit` + `pixels` desktop window path.
- A `cpal` audio output path.
- Keyboard mapping for NES controls and emulator controls.
- Unit tests for parser behavior, mapper behavior, CPU basics, bus routing, PPU state, controller reads, save-state validation, and app wiring.

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
- [ ] Complete target-ROM CPU behavior and interrupt accuracy.
- [ ] Complete PPU rendering behavior needed for correct gameplay.
- [ ] Complete APU sound behavior and stable synchronization.
- [ ] Persist save states to disk across sessions.
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
| Select | Right Shift |
| Pause / Resume | Space or P |
| Save State | F5 |
| Load State | F9 |
| Select Slot | 1, 2, 3 |
| Exit | Escape |

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
