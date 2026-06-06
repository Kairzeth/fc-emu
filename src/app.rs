use crate::{
    emulator::Emulator,
    input::{AppControlAction, Button, KeyMapping, KeyboardKey, SaveSlot, default_key_mapping},
    rom::Rom,
    save_state::{SaveState, validate_slot},
    window,
};
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use tracing::info;

pub struct App {
    emulator: Emulator,
    rom_path: PathBuf,
    paused: bool,
    current_slot: u8,
    last_state: Option<SaveState>,
    should_exit: bool,
}

impl App {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let rom_path = path.as_ref().to_path_buf();
        if !rom_path.exists() {
            return Err(anyhow!("ROM file '{}' does not exist", rom_path.display()));
        }

        let rom = Rom::from_path(&rom_path)
            .with_context(|| format!("failed to load ROM '{}'", rom_path.display()))?;
        let rom_name = rom_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("rom")
            .to_string();
        info!(
            rom = %rom_name,
            mapper = rom.mapper,
            prg_banks = rom.prg_banks,
            chr_banks = rom.chr_banks,
            "loaded ROM"
        );
        let emulator = Emulator::new(rom, rom_name).map_err(|err| anyhow!(err))?;

        Ok(Self {
            emulator,
            rom_path,
            paused: false,
            current_slot: 1,
            last_state: None,
            should_exit: false,
        })
    }

    pub fn run(self) -> Result<()> {
        window::run(self)
    }

    pub fn tick(&mut self) {
        if !self.paused {
            self.emulator.step_frame();
        }
    }

    pub fn handle_action(&mut self, action: AppControlAction) -> Result<()> {
        match action {
            AppControlAction::Pause => self.paused = true,
            AppControlAction::Resume => self.paused = false,
            AppControlAction::TogglePause => self.paused = !self.paused,
            AppControlAction::SaveState => self.save_current_state(),
            AppControlAction::LoadState => self.load_current_state(),
            AppControlAction::SelectSaveSlot(SaveSlot::Slot(slot)) => {
                validate_slot(slot)?;
                self.current_slot = slot;
            }
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyboardKey, pressed: bool) -> Result<()> {
        match default_key_mapping(key) {
            Some(KeyMapping::Controller(button)) => self.set_button(button, pressed),
            Some(KeyMapping::App(action)) if pressed => self.handle_action(action)?,
            Some(KeyMapping::App(_)) | None => {}
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.emulator.reset();
    }

    pub fn save_current_state(&mut self) {
        self.last_state = Some(self.emulator.save_state());
        info!(slot = self.current_slot, "saved in-memory state");
    }

    pub fn load_current_state(&mut self) {
        if let Some(state) = &self.last_state {
            self.emulator.load_state(state);
            info!(slot = self.current_slot, "loaded in-memory state");
        }
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.emulator.set_button(button, pressed);
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.emulator.frame_buffer()
    }

    pub fn drain_audio_samples(&mut self, output: &mut Vec<f32>) {
        self.emulator.drain_audio_samples(output);
    }

    pub fn window_title(&self) -> String {
        let name = self
            .rom_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fc-emu");
        if self.paused {
            format!("{name} - Paused")
        } else {
            name.to_string()
        }
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn current_slot(&self) -> u8 {
        self.current_slot
    }

    pub fn rom_path(&self) -> &Path {
        &self.rom_path
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn request_exit(&mut self) {
        self.should_exit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::AppControlAction;

    #[test]
    fn rejects_missing_rom_with_clear_message() {
        let err = App::new("rom/does-not-exist.nes").err().unwrap();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn toggles_pause_state() {
        let mut app = App::new(crate::DEFAULT_ROM_PATH).unwrap();
        assert!(!app.paused());
        app.handle_action(AppControlAction::TogglePause).unwrap();
        assert!(app.paused());
    }

    #[test]
    fn changes_save_slot_only_for_valid_slots() {
        let mut app = App::new(crate::DEFAULT_ROM_PATH).unwrap();
        app.handle_action(AppControlAction::SelectSaveSlot(SaveSlot::Slot(3)))
            .unwrap();
        assert_eq!(app.current_slot(), 3);
        assert!(
            app.handle_action(AppControlAction::SelectSaveSlot(SaveSlot::Slot(4)))
                .is_err()
        );
    }
}
