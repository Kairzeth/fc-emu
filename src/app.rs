use crate::{
    emulator::Emulator,
    input::{AppControlAction, Button, KeyMapping, KeyboardKey, SaveSlot, default_key_mapping},
    rom::Rom,
    save_state::{
        SaveState, read_state, save_path_for_rom, validate_slot, validate_state, write_state,
    },
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
    audio_reset_requested: bool,
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
            audio_reset_requested: false,
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
            AppControlAction::SaveState => self.save_current_state()?,
            AppControlAction::LoadState => self.load_current_state()?,
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
        self.audio_reset_requested = true;
    }

    pub fn save_current_state(&mut self) -> Result<()> {
        let state = self.emulator.save_state();
        let path = save_path_for_rom(&self.rom_path, self.current_slot)?;
        write_state(&path, &state)
            .with_context(|| format!("failed to save state to '{}'", path.display()))?;
        self.last_state = Some(state);
        info!(slot = self.current_slot, path = %path.display(), "saved state");
        Ok(())
    }

    pub fn load_current_state(&mut self) -> Result<()> {
        let path = save_path_for_rom(&self.rom_path, self.current_slot)?;
        let state = read_state(&path)
            .with_context(|| format!("failed to load state from '{}'", path.display()))?;
        validate_state(&state, &self.rom_name())?;
        self.emulator.load_state(&state);
        self.last_state = Some(state);
        self.audio_reset_requested = true;
        info!(slot = self.current_slot, path = %path.display(), "loaded state");
        Ok(())
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

    fn rom_name(&self) -> String {
        self.rom_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("rom")
            .to_string()
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn request_exit(&mut self) {
        self.should_exit = true;
    }

    pub fn take_audio_reset_requested(&mut self) -> bool {
        let requested = self.audio_reset_requested;
        self.audio_reset_requested = false;
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{AppControlAction, KeyboardKey};
    use crate::save_state::{read_state, save_path_for_rom};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn reset_requests_audio_queue_clear_once() {
        let mut app = App::new(crate::DEFAULT_ROM_PATH).unwrap();

        app.reset();

        assert!(app.take_audio_reset_requested());
        assert!(!app.take_audio_reset_requested());
    }

    #[test]
    fn save_action_writes_current_slot_to_disk() {
        let temp_rom = temp_rom_copy();
        let mut app = App::new(&temp_rom).unwrap();
        app.handle_action(AppControlAction::SelectSaveSlot(SaveSlot::Slot(2)))
            .unwrap();
        let path = save_path_for_rom(app.rom_path(), app.current_slot()).unwrap();
        let _ = fs::remove_file(&path);

        app.handle_action(AppControlAction::SaveState).unwrap();

        let state = read_state(&path).unwrap();
        assert_eq!(
            state.rom_name,
            temp_rom.file_name().unwrap().to_string_lossy()
        );
        cleanup_temp_rom_and_save(temp_rom, path);
    }

    #[test]
    fn load_action_reports_missing_slot_file() {
        let temp_rom = temp_rom_copy();
        let mut app = App::new(&temp_rom).unwrap();
        app.handle_action(AppControlAction::SelectSaveSlot(SaveSlot::Slot(3)))
            .unwrap();
        let path = save_path_for_rom(app.rom_path(), app.current_slot()).unwrap();
        let _ = fs::remove_file(&path);

        let err = app
            .handle_action(AppControlAction::LoadState)
            .err()
            .unwrap();
        let error_chain = format!("{err:#}");

        assert!(error_chain.contains("failed to load state"));
        assert!(error_chain.contains("does not exist"));
        cleanup_temp_rom_and_save(temp_rom, path);
    }

    #[test]
    fn f5_and_f9_use_current_disk_save_slot() {
        let temp_rom = temp_rom_copy();
        let mut app = App::new(&temp_rom).unwrap();
        app.handle_key(KeyboardKey::Digit(1), true).unwrap();
        let path = save_path_for_rom(app.rom_path(), app.current_slot()).unwrap();
        let _ = fs::remove_file(&path);

        app.tick();
        app.handle_key(KeyboardKey::F5, true).unwrap();
        let saved = read_state(&path).unwrap();
        app.tick();
        app.handle_key(KeyboardKey::F9, true).unwrap();

        assert_eq!(app.last_state.as_ref(), Some(&saved));
        assert!(app.take_audio_reset_requested());
        cleanup_temp_rom_and_save(temp_rom, path);
    }

    fn temp_rom_copy() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fc-emu-app-test-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let rom = dir.join(format!("test-rom-{unique}.nes"));
        fs::copy(crate::DEFAULT_ROM_PATH, &rom).unwrap();
        rom
    }

    fn cleanup_temp_rom_and_save(rom: PathBuf, save: PathBuf) {
        let _ = fs::remove_file(save);
        if let Some(parent) = rom.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
