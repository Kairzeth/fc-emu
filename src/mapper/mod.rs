pub mod mapper0;

use crate::rom::Rom;
use mapper0::Mapper0;
use std::fmt;

pub trait Mapper {
    fn cpu_read(&self, addr: u16) -> Option<u8>;
    fn cpu_write(&mut self, addr: u16, value: u8) -> bool;
    fn ppu_read(&self, addr: u16) -> Option<u8>;
    fn ppu_write(&mut self, addr: u16, value: u8) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperError {
    UnsupportedMapper(u8),
}

pub fn from_rom(rom: Rom) -> Result<Box<dyn Mapper>, MapperError> {
    match rom.mapper {
        0 => Ok(Box::new(Mapper0::from_rom_data(rom.prg_rom, rom.chr_rom))),
        mapper => Err(MapperError::UnsupportedMapper(mapper)),
    }
}

pub fn create_mapper(rom: &Rom) -> Result<Box<dyn Mapper>, MapperError> {
    from_rom(rom.clone())
}

impl fmt::Display for MapperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedMapper(mapper) => {
                write!(f, "mapper {mapper} is not supported in the first stage")
            }
        }
    }
}

impl std::error::Error for MapperError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::{Mirroring, Rom};

    fn rom_with_mapper(mapper: u8) -> Rom {
        Rom {
            prg_rom: vec![0; 16 * 1024],
            chr_rom: vec![0; 8 * 1024],
            mapper,
            prg_banks: 1,
            chr_banks: 1,
            mirroring: Mirroring::Horizontal,
            has_trainer: false,
        }
    }

    #[test]
    fn creates_mapper0_for_nrom() {
        assert!(from_rom(rom_with_mapper(0)).is_ok());
    }

    #[test]
    fn rejects_unsupported_mapper() {
        match from_rom(rom_with_mapper(1)) {
            Err(error) => assert_eq!(error, MapperError::UnsupportedMapper(1)),
            Ok(_) => panic!("expected mapper 1 to be rejected"),
        }
    }
}
