use crate::{apu::ApuState, bus::BusState, cpu::CpuState, ppu::PpuState};
use std::path::{Path, PathBuf};

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct SaveState {
    pub version: u32,
    pub rom_name: String,
    pub cpu: CpuState,
    pub bus: BusState,
    pub ppu: PpuState,
    pub apu: ApuState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStateError {
    InvalidSlot(u8),
    IncompatibleVersion { expected: u32, actual: u32 },
    RomMismatch { expected: String, actual: String },
}

impl std::fmt::Display for SaveStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSlot(slot) => {
                write!(f, "save slot {slot} is invalid; use slot 1, 2, or 3")
            }
            Self::IncompatibleVersion { expected, actual } => {
                write!(
                    f,
                    "save state version {actual} is not compatible with version {expected}"
                )
            }
            Self::RomMismatch { expected, actual } => {
                write!(f, "save state belongs to ROM '{actual}', not '{expected}'")
            }
        }
    }
}

impl std::error::Error for SaveStateError {}

pub fn validate_slot(slot: u8) -> Result<(), SaveStateError> {
    match slot {
        1..=3 => Ok(()),
        _ => Err(SaveStateError::InvalidSlot(slot)),
    }
}

pub fn save_path_for_rom(rom_path: &Path, slot: u8) -> Result<PathBuf, SaveStateError> {
    validate_slot(slot)?;
    let stem = rom_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("rom");
    Ok(PathBuf::from("saves").join(format!("{stem}.slot{slot}.state")))
}

pub fn validate_state(state: &SaveState, rom_name: &str) -> Result<(), SaveStateError> {
    if state.version != SAVE_VERSION {
        return Err(SaveStateError::IncompatibleVersion {
            expected: SAVE_VERSION,
            actual: state.version,
        });
    }
    if state.rom_name != rom_name {
        return Err(SaveStateError::RomMismatch {
            expected: rom_name.to_string(),
            actual: state.rom_name.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_path_uses_rom_stem_and_slot() {
        let path =
            save_path_for_rom(Path::new("rom/Super Mario Bros. (Japan, USA).nes"), 2).unwrap();
        assert_eq!(
            path,
            PathBuf::from("saves/Super Mario Bros. (Japan, USA).slot2.state")
        );
    }

    #[test]
    fn rejects_invalid_slot() {
        assert_eq!(validate_slot(4), Err(SaveStateError::InvalidSlot(4)));
    }

    fn minimal_state(version: u32, rom_name: &str) -> SaveState {
        SaveState {
            version,
            rom_name: rom_name.to_string(),
            cpu: crate::cpu::Cpu::default().snapshot(),
            bus: crate::bus::BusState {
                cpu_ram: [0; crate::bus::CPU_RAM_SIZE],
                controller1: crate::input::Controller::default(),
                oam_dma: crate::bus::OamDmaState::default(),
            },
            ppu: crate::ppu::Ppu::new(crate::rom::Mirroring::Horizontal).snapshot(),
            apu: crate::apu::Apu::default().snapshot(),
        }
    }

    #[test]
    fn rejects_incompatible_versions() {
        assert!(matches!(
            validate_state(&minimal_state(999, "a.nes"), "a.nes"),
            Err(SaveStateError::IncompatibleVersion { .. })
        ));
    }

    #[test]
    fn rejects_rom_mismatch() {
        assert!(matches!(
            validate_state(&minimal_state(SAVE_VERSION, "a.nes"), "b.nes"),
            Err(SaveStateError::RomMismatch { .. })
        ));
    }
}
