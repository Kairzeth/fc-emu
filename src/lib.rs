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

use std::path::Path;

pub const DEFAULT_ROM_PATH: &str = "rom/Super Mario Bros. (Japan, USA).nes";

pub fn select_rom_path(args: impl IntoIterator<Item = String>) -> String {
    args.into_iter().nth(1).unwrap_or_else(default_rom_path)
}

fn default_rom_path() -> String {
    if Path::new(DEFAULT_ROM_PATH).exists() {
        return DEFAULT_ROM_PATH.to_string();
    }

    bundled_rom_path().unwrap_or_else(|| DEFAULT_ROM_PATH.to_string())
}

fn bundled_rom_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let contents_dir = exe.parent()?.parent()?;
    let path = contents_dir.join("Resources").join(DEFAULT_ROM_PATH);
    path.exists().then(|| path.to_string_lossy().into_owned())
}
