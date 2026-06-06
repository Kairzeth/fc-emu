pub mod app;
pub mod apu;
pub mod bus;
pub mod cpu;
pub mod emulator;
pub mod input;
pub mod mapper;
pub mod ppu;
pub mod rom;
pub mod save_state;
pub mod window;

pub const DEFAULT_ROM_PATH: &str = "rom/Super Mario Bros. (Japan, USA).nes";

pub fn select_rom_path(args: impl IntoIterator<Item = String>) -> String {
    args.into_iter()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ROM_PATH.to_string())
}
