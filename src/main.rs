use anyhow::Context;
use fc_emu::{app::App, select_rom_path};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let rom_path = select_rom_path(std::env::args());

    App::new(&rom_path)
        .with_context(|| format!("failed to initialize app for ROM '{rom_path}'"))?
        .run()
}

#[cfg(test)]
mod tests {
    use fc_emu::{DEFAULT_ROM_PATH, select_rom_path};

    #[test]
    fn defaults_to_bundled_rom_path() {
        assert_eq!(
            select_rom_path(["fc-emu".to_string()]),
            DEFAULT_ROM_PATH.to_string()
        );
    }

    #[test]
    fn accepts_explicit_rom_path() {
        assert_eq!(
            select_rom_path(["fc-emu".to_string(), "custom.nes".to_string()]),
            "custom.nes"
        );
    }
}
