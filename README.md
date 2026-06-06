# fc-emu

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

---

# fc-emu 中文说明

`fc-emu` 是一个使用 Rust 开发的 FC/NES 模拟器项目。第一阶段聚焦一个明确目标：加载本地的 `Super Mario Bros. (Japan, USA).nes` ROM，打开桌面窗口，并逐步实现画面、键盘输入、声音、暂停和即时存档，最终达到可以完整游玩《Super Mario Bros.》的程度。

项目当前刻意保持范围收敛。第一阶段不追求完整通用的 FC/NES 兼容性，而是先围绕目标 ROM 搭建清晰、可测试、可扩展的模拟器核心，再在后续阶段扩展更多 Mapper、渲染精度、音频精度、调试工具和兼容性。

## 当前状态

代码库已经具备第一版模拟器架构：

- ROM 加载与 iNES 解析。
- Mapper 0 / NROM 卡带映射。
- CPU、PPU、APU、Bus、Input、Save State、Window、App 等模块。
- 基于 `winit` + `pixels` 的桌面窗口路径。
- 基于 `cpal` 的音频输出路径。
- FC/NES 手柄按键与模拟器控制快捷键映射。
- 覆盖 ROM 解析、Mapper、CPU 基础行为、Bus 路由、PPU 状态、手柄读取、存档校验和 App 组装的单元测试。

项目仍在开发中。当前版本适合继续实现与验证，不是已经打磨完成的通用模拟器发布版。

## 整体目标

整体目标是用 Rust 构建一个可游玩、可维护的 FC/NES 模拟器。近期目标是让《Super Mario Bros.》在开发者电脑上流畅运行，同时让模拟器核心逻辑与窗口、输入、音频等平台相关代码解耦，便于测试和后续扩展。

在第一阶段目标 ROM 稳定运行后，项目可以继续扩展到更多 ROM 兼容、更多 Mapper、更准确的时序、更完善的调试工具、性能观测和更完整的用户体验。

## 第一阶段范围

- [x] 完成 Rust 项目初始化与模块结构。
- [x] 支持默认 ROM 路径和命令行传入 ROM 路径。
- [x] 解析 iNES ROM 头并提取 PRG/CHR 数据。
- [x] 实现 Mapper 0 / NROM 行为。
- [x] 实现 Bus 对 CPU RAM、PPU 寄存器、APU、手柄输入、DMA 和卡带访问的路由。
- [x] 实现键盘到 FC/NES 手柄输入的映射。
- [x] 实现应用层组装、暂停控制、保存/读取动作和窗口标题更新。
- [x] 添加即时存档数据结构和校验逻辑。
- [x] 添加基于 `winit` 和 `pixels` 的桌面窗口路径。
- [x] 添加基于 `cpal` 的基础音频输出路径。
- [ ] 补全目标 ROM 所需的 CPU 行为和中断精度。
- [ ] 补全正确游玩所需的 PPU 渲染行为。
- [ ] 补全 APU 声音行为和稳定同步。
- [ ] 将即时存档持久化到磁盘。
- [ ] 完成暂停、菜单交互和完整游玩验收。
- [ ] 验证目标 ROM 可以完整游玩。

## 环境要求

- Rust 工具链与 Cargo。
- 一个本地 `.nes` ROM 文件用于开发和测试。

仓库不包含 ROM 文件。请将你合法拥有的 ROM 放到：

```bash
rom/Super Mario Bros. (Japan, USA).nes
```

也可以在启动时显式传入 ROM 路径。

## 运行方式

使用默认 ROM 路径运行：

```bash
cargo run
```

指定 ROM 路径运行：

```bash
cargo run -- path/to/game.nes
```

## 按键

| FC/NES 按键 | 键盘 |
| --- | --- |
| 上 | 方向键上 |
| 下 | 方向键下 |
| 左 | 方向键左 |
| 右 | 方向键右 |
| A | X |
| B | Z |
| Start | Enter |
| Select | Right Shift |
| 暂停 / 恢复 | Space 或 P |
| 保存即时存档 | F5 |
| 读取即时存档 | F9 |
| 选择存档槽 | 1、2、3 |
| 退出 | Escape |

## 开发

运行测试：

```bash
cargo test
```

当前测试覆盖已经实现的主要底层行为。

## 架构

项目按模拟器核心模块拆分：

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

模拟器核心与外层应用、窗口逻辑保持分离，便于独立测试和后续维护。
