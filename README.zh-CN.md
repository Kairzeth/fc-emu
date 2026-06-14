<div align="center">

# A Rust FC/NES emulator

![Rust](https://img.shields.io/badge/Rust-2024-b7410e?style=for-the-badge&logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-stage%201%20WIP-f4b400?style=for-the-badge)
![Target](https://img.shields.io/badge/target-Super%20Mario%20Bros.-e52521?style=for-the-badge)
![Mapper](https://img.shields.io/badge/mapper-0%20%2F%20NROM-3366cc?style=for-the-badge)
![Tests](https://img.shields.io/badge/tests-74%20passing-2ea44f?style=for-the-badge)

[English](README.md) | [中文](README.zh-CN.md)

</div>

`fc-emu` 是一个使用 Rust 开发的 FC/NES 模拟器项目。第一阶段聚焦一个明确目标：加载本地的 `Super Mario Bros. (Japan, USA).nes` ROM，打开桌面窗口，并逐步实现画面、键盘输入、声音、暂停和即时存档，最终达到可以完整游玩《Super Mario Bros.》的程度。

项目当前刻意保持范围收敛。第一阶段不追求完整通用的 FC/NES 兼容性，而是先围绕目标 ROM 搭建清晰、可测试、可扩展的模拟器核心，再在后续阶段扩展更多 Mapper、渲染精度、音频精度、调试工具和兼容性。

## 项目速览

| 领域 | 当前选择 |
| --- | --- |
| 语言 | Rust 2024 |
| 第一目标 | `Super Mario Bros. (Japan, USA).nes` |
| 卡带范围 | 优先支持 Mapper 0 / NROM |
| 窗口方案 | `winit` + `pixels` |
| 音频方案 | `cpal` |
| 测试方式 | Rust 单元测试 |

## 当前状态

代码库已经具备第一版模拟器架构：

- ROM 加载与 iNES 解析。
- Mapper 0 / NROM 卡带映射。
- CPU、PPU、APU、Bus、Input、Save State、Window、App 等模块。
- 基于 `winit` + `pixels` 的桌面窗口路径。
- 基于 `cpal` 的音频输出路径，包含基础 pulse、triangle、noise 混音。
- FC/NES 手柄按键与模拟器控制快捷键映射。
- 支持写入 `saves/` 目录的 3 槽位磁盘即时存档。
- 覆盖 ROM 解析、Mapper、CPU 行为、Bus 路由、PPU 基础渲染、手柄读取、存档持久化以及 App/Window 组装的单元测试。

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
- [x] 将即时存档持久化到磁盘。
- [ ] 补全目标 ROM 所需的 CPU 行为和中断精度。
- [ ] 补全正确游玩所需的 PPU 渲染行为。
- [ ] 补全 APU 声音行为和稳定同步。
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
| Select | Right Shift、S 或 Tab |
| 暂停 / 恢复 | Space 或 P |
| 保存即时存档 | F 或 F5 |
| 读取即时存档 | L 或 F9 |
| 选择存档槽 | 1、2、3 |
| 退出 | Escape |

即时存档会写入 `saves/` 目录，并按 ROM 文件名和槽位编号区分。

## 已知问题

- 声音仍不正确，当前 APU 还不能还原《Super Mario Bros.》的原版音乐。
- 顶出蘑菇后，蘑菇完全出现时画面仍可能卡死，后续需要继续修 PPU/CPU 时序相关问题。

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
