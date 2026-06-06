use super::Mapper;

const PRG_ROM_BANK_LEN: usize = 16 * 1024;
const CHR_BANK_LEN: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapper0 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_rom: bool,
}

impl Mapper0 {
    pub fn new(prg_rom: Vec<u8>, chr: Vec<u8>, chr_is_rom: bool) -> Self {
        let chr = if chr.is_empty() {
            vec![0; CHR_BANK_LEN]
        } else {
            chr
        };

        Self {
            prg_rom,
            chr,
            chr_is_rom,
        }
    }

    pub fn from_rom_data(prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        let chr_is_rom = !chr_rom.is_empty();
        Self::new(prg_rom, chr_rom, chr_is_rom)
    }

    fn prg_index(&self, addr: u16) -> Option<usize> {
        if !(0x8000..=0xFFFF).contains(&addr) || self.prg_rom.is_empty() {
            return None;
        }

        let offset = (addr - 0x8000) as usize;
        Some(if self.prg_rom.len() <= PRG_ROM_BANK_LEN {
            offset % PRG_ROM_BANK_LEN
        } else {
            offset
        })
    }
}

impl Mapper for Mapper0 {
    fn cpu_read(&self, addr: u16) -> Option<u8> {
        self.prg_index(addr)
            .and_then(|index| self.prg_rom.get(index).copied())
    }

    fn cpu_write(&mut self, addr: u16, _value: u8) -> bool {
        (0x8000..=0xFFFF).contains(&addr)
    }

    fn ppu_read(&self, addr: u16) -> Option<u8> {
        if !(0x0000..=0x1FFF).contains(&addr) {
            return None;
        }

        self.chr.get(addr as usize).copied()
    }

    fn ppu_write(&mut self, addr: u16, value: u8) -> bool {
        if !(0x0000..=0x1FFF).contains(&addr) || self.chr_is_rom {
            return false;
        }

        self.chr[addr as usize] = value;
        true
    }
}

impl crate::bus::MapperBusDevice for Mapper0 {
    fn cpu_read(&mut self, addr: u16) -> Option<u8> {
        Mapper::cpu_read(self, addr)
    }

    fn cpu_write(&mut self, addr: u16, value: u8) -> bool {
        Mapper::cpu_write(self, addr, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapper_with_prg(prg_len: usize) -> Mapper0 {
        let prg_rom = (0..prg_len).map(|i| (i / 1024) as u8).collect();
        let chr_rom = (0..CHR_BANK_LEN).map(|i| (i % 251) as u8).collect();
        Mapper0::new(prg_rom, chr_rom, true)
    }

    #[test]
    fn mirrors_16kb_prg_rom() {
        let mapper = mapper_with_prg(PRG_ROM_BANK_LEN);

        assert_eq!(mapper.cpu_read(0x8000), mapper.cpu_read(0xC000));
        assert_eq!(mapper.cpu_read(0xBFFF), mapper.cpu_read(0xFFFF));
    }

    #[test]
    fn maps_32kb_prg_rom_directly() {
        let mapper = mapper_with_prg(2 * PRG_ROM_BANK_LEN);

        assert_eq!(mapper.cpu_read(0x8000), Some(0));
        assert_eq!(mapper.cpu_read(0xC000), Some(16));
        assert_ne!(mapper.cpu_read(0x8000), mapper.cpu_read(0xC000));
    }

    #[test]
    fn maps_ppu_chr_reads() {
        let mapper = mapper_with_prg(PRG_ROM_BANK_LEN);

        assert_eq!(mapper.ppu_read(0x0000), Some(0));
        assert_eq!(mapper.ppu_read(0x0001), Some(1));
        assert_eq!(mapper.ppu_read(0x1FFF), Some((8191 % 251) as u8));
    }

    #[test]
    fn returns_none_or_false_outside_cartridge_space() {
        let mut mapper = mapper_with_prg(PRG_ROM_BANK_LEN);

        assert_eq!(mapper.cpu_read(0x7FFF), None);
        assert!(!mapper.cpu_write(0x7FFF, 0xAA));
        assert_eq!(mapper.ppu_read(0x2000), None);
        assert!(!mapper.ppu_write(0x2000, 0xAA));
    }

    #[test]
    fn treats_chr_rom_as_read_only() {
        let mut mapper = mapper_with_prg(PRG_ROM_BANK_LEN);

        assert!(!mapper.ppu_write(0x0000, 0xAA));
        assert_eq!(mapper.ppu_read(0x0000), Some(0));
    }

    #[test]
    fn supports_chr_ram_when_rom_has_no_chr_banks() {
        let mut mapper = Mapper0::new(vec![0; PRG_ROM_BANK_LEN], Vec::new(), false);

        assert!(mapper.ppu_write(0x0000, 0xAA));
        assert_eq!(mapper.ppu_read(0x0000), Some(0xAA));
    }
}
