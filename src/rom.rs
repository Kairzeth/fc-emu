use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

const INES_MAGIC: &[u8; 4] = b"NES\x1A";
const HEADER_LEN: usize = 16;
const TRAINER_LEN: usize = 512;
const PRG_ROM_BANK_LEN: usize = 16 * 1024;
const CHR_ROM_BANK_LEN: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rom {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
    pub prg_banks: u8,
    pub chr_banks: u8,
    pub mirroring: Mirroring,
    pub has_trainer: bool,
}

#[derive(Debug)]
pub enum RomError {
    Io(io::Error),
    FileTooSmall {
        len: usize,
    },
    InvalidMagic {
        actual: [u8; 4],
    },
    UnsupportedMapper(u8),
    Truncated {
        section: &'static str,
        expected_at_least: usize,
        actual: usize,
    },
}

impl Rom {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, RomError> {
        let bytes = fs::read(path).map_err(RomError::Io)?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RomError> {
        if bytes.len() < HEADER_LEN {
            return Err(RomError::FileTooSmall { len: bytes.len() });
        }

        let actual_magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if &actual_magic != INES_MAGIC {
            return Err(RomError::InvalidMagic {
                actual: actual_magic,
            });
        }

        let prg_banks = bytes[4];
        let chr_banks = bytes[5];
        let flags6 = bytes[6];
        let flags7 = bytes[7];
        let mapper = (flags7 & 0xF0) | (flags6 >> 4);

        if mapper != 0 {
            return Err(RomError::UnsupportedMapper(mapper));
        }

        let mirroring = if flags6 & 0b1000 != 0 {
            Mirroring::FourScreen
        } else if flags6 & 0b0001 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let has_trainer = flags6 & 0b0100 != 0;
        let trainer_len = if has_trainer { TRAINER_LEN } else { 0 };
        let prg_start = HEADER_LEN + trainer_len;
        let prg_len = prg_banks as usize * PRG_ROM_BANK_LEN;
        let chr_start = prg_start + prg_len;
        let chr_len = chr_banks as usize * CHR_ROM_BANK_LEN;
        let expected_len = chr_start + chr_len;

        if bytes.len() < prg_start {
            return Err(RomError::Truncated {
                section: "trainer",
                expected_at_least: prg_start,
                actual: bytes.len(),
            });
        }

        if bytes.len() < chr_start {
            return Err(RomError::Truncated {
                section: "PRG ROM",
                expected_at_least: chr_start,
                actual: bytes.len(),
            });
        }

        if bytes.len() < expected_len {
            return Err(RomError::Truncated {
                section: "CHR ROM",
                expected_at_least: expected_len,
                actual: bytes.len(),
            });
        }

        Ok(Self {
            prg_rom: bytes[prg_start..chr_start].to_vec(),
            chr_rom: bytes[chr_start..expected_len].to_vec(),
            mapper,
            prg_banks,
            chr_banks,
            mirroring,
            has_trainer,
        })
    }
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read ROM: {error}"),
            Self::FileTooSmall { len } => {
                write!(f, "ROM is too small to contain an iNES header: {len} bytes")
            }
            Self::InvalidMagic { actual } => {
                write!(
                    f,
                    "invalid iNES magic: expected NES<EOF>, got {actual:02X?}"
                )
            }
            Self::UnsupportedMapper(mapper) => {
                write!(f, "mapper {mapper} is not supported in the first stage")
            }
            Self::Truncated {
                section,
                expected_at_least,
                actual,
            } => write!(
                f,
                "ROM is truncated while reading {section}: expected at least {expected_at_least} bytes, got {actual}"
            ),
        }
    }
}

impl std::error::Error for RomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ines_bytes(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut bytes = vec![0; HEADER_LEN];
        bytes[0..4].copy_from_slice(INES_MAGIC);
        bytes[4] = prg_banks;
        bytes[5] = chr_banks;
        bytes[6] = flags6;
        bytes[7] = flags7;

        if flags6 & 0b0100 != 0 {
            bytes.extend(vec![0xEE; TRAINER_LEN]);
        }

        bytes.extend((0..prg_banks as usize * PRG_ROM_BANK_LEN).map(|i| i as u8));
        bytes.extend((0..chr_banks as usize * CHR_ROM_BANK_LEN).map(|i| (i as u8).wrapping_add(1)));
        bytes
    }

    #[test]
    fn parses_valid_ines_magic() {
        let rom = Rom::from_bytes(&ines_bytes(1, 1, 0, 0)).unwrap();

        assert_eq!(rom.mapper, 0);
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut bytes = ines_bytes(1, 1, 0, 0);
        bytes[0..4].copy_from_slice(b"NOPE");

        assert!(matches!(
            Rom::from_bytes(&bytes),
            Err(RomError::InvalidMagic { .. })
        ));
    }

    #[test]
    fn parses_prg_and_chr_sizes() {
        let rom = Rom::from_bytes(&ines_bytes(2, 1, 0, 0)).unwrap();

        assert_eq!(rom.prg_banks, 2);
        assert_eq!(rom.chr_banks, 1);
        assert_eq!(rom.prg_rom.len(), 2 * PRG_ROM_BANK_LEN);
        assert_eq!(rom.chr_rom.len(), CHR_ROM_BANK_LEN);
    }

    #[test]
    fn parses_mapper_number_from_header_nibbles() {
        let bytes = ines_bytes(1, 1, 0b0001_0000, 0);

        assert!(matches!(
            Rom::from_bytes(&bytes),
            Err(RomError::UnsupportedMapper(1))
        ));
    }

    #[test]
    fn parses_mirroring() {
        assert_eq!(
            Rom::from_bytes(&ines_bytes(1, 1, 0, 0)).unwrap().mirroring,
            Mirroring::Horizontal
        );
        assert_eq!(
            Rom::from_bytes(&ines_bytes(1, 1, 0b0001, 0))
                .unwrap()
                .mirroring,
            Mirroring::Vertical
        );
        assert_eq!(
            Rom::from_bytes(&ines_bytes(1, 1, 0b1000, 0))
                .unwrap()
                .mirroring,
            Mirroring::FourScreen
        );
    }

    #[test]
    fn skips_trainer_before_prg_rom() {
        let rom = Rom::from_bytes(&ines_bytes(1, 1, 0b0100, 0)).unwrap();

        assert!(rom.has_trainer);
        assert_eq!(rom.prg_rom[0], 0);
        assert_eq!(rom.prg_rom[1], 1);
        assert_eq!(rom.chr_rom[0], 1);
    }
}
