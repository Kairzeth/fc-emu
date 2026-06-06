use crate::{mapper::Mapper, rom::Mirroring};

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;
pub const FRAMEBUFFER_SIZE: usize = WIDTH * HEIGHT * 4;

#[derive(Clone, Debug)]
pub struct Ppu {
    registers: [u8; 8],
    vram: [u8; 0x800],
    palette: [u8; 0x20],
    oam: [u8; 0x100],
    oam_addr: u8,
    scanline: i16,
    dot: u16,
    frame_ready: bool,
    nmi_pending: bool,
    address_latch: bool,
    temp_addr: u16,
    vram_addr: u16,
    read_buffer: u8,
    mirroring: Mirroring,
    framebuffer: Vec<u8>,
}

impl Ppu {
    pub fn new(mirroring: Mirroring) -> Self {
        let mut ppu = Self {
            registers: [0; 8],
            vram: [0; 0x800],
            palette: [0; 0x20],
            oam: [0; 0x100],
            oam_addr: 0,
            scanline: 0,
            dot: 0,
            frame_ready: false,
            nmi_pending: false,
            address_latch: false,
            temp_addr: 0,
            vram_addr: 0,
            read_buffer: 0,
            mirroring,
            framebuffer: vec![0; FRAMEBUFFER_SIZE],
        };
        ppu.draw_placeholder_frame();
        ppu
    }

    pub fn cpu_read_register(&mut self, addr: u16, mapper: &dyn Mapper) -> u8 {
        match addr & 7 {
            2 => {
                let status = self.registers[2];
                self.registers[2] &= 0x7f;
                self.address_latch = false;
                status
            }
            4 => self.oam[self.oam_addr as usize],
            7 => self.read_ppudata(mapper),
            _ => 0,
        }
    }

    pub fn cpu_write_register(&mut self, addr: u16, value: u8, mapper: &mut dyn Mapper) {
        match addr & 7 {
            0 | 1 => self.registers[(addr & 7) as usize] = value,
            3 => self.oam_addr = value,
            4 => {
                self.oam[self.oam_addr as usize] = value;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                self.address_latch = !self.address_latch;
            }
            6 => {
                if !self.address_latch {
                    self.temp_addr = (u16::from(value & 0x3f)) << 8;
                    self.address_latch = true;
                } else {
                    self.vram_addr = self.temp_addr | u16::from(value);
                    self.address_latch = false;
                }
            }
            7 => self.write_ppudata(value, mapper),
            _ => {}
        }
    }

    pub fn write_oam_dma(&mut self, bytes: &[u8; 256]) {
        for byte in bytes {
            self.oam[self.oam_addr as usize] = *byte;
            self.oam_addr = self.oam_addr.wrapping_add(1);
        }
    }

    pub fn step(&mut self, cycles: usize) {
        for _ in 0..cycles {
            self.dot += 1;
            if self.dot >= 341 {
                self.dot = 0;
                self.scanline += 1;
                if self.scanline == 241 {
                    self.registers[2] |= 0x80;
                    if self.registers[0] & 0x80 != 0 {
                        self.nmi_pending = true;
                    }
                    self.frame_ready = true;
                    self.draw_placeholder_frame();
                } else if self.scanline >= 262 {
                    self.scanline = 0;
                    self.registers[2] &= 0x1f;
                    self.frame_ready = false;
                }
            }
        }
    }

    pub fn frame_ready(&self) -> bool {
        self.frame_ready
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn take_frame(&mut self) -> &[u8] {
        self.frame_ready = false;
        &self.framebuffer
    }

    pub fn poll_nmi(&mut self) -> bool {
        let pending = self.nmi_pending;
        self.nmi_pending = false;
        pending
    }

    pub fn snapshot(&self) -> PpuState {
        PpuState {
            registers: self.registers,
            vram: self.vram.to_vec(),
            palette: self.palette,
            oam: self.oam,
            oam_addr: self.oam_addr,
            scanline: self.scanline,
            dot: self.dot,
            frame_ready: self.frame_ready,
            nmi_pending: self.nmi_pending,
            address_latch: self.address_latch,
            temp_addr: self.temp_addr,
            vram_addr: self.vram_addr,
            read_buffer: self.read_buffer,
            framebuffer: self.framebuffer.clone(),
        }
    }

    pub fn restore(&mut self, state: &PpuState) {
        self.registers = state.registers;
        self.vram.copy_from_slice(&state.vram[..0x800]);
        self.palette = state.palette;
        self.oam = state.oam;
        self.oam_addr = state.oam_addr;
        self.scanline = state.scanline;
        self.dot = state.dot;
        self.frame_ready = state.frame_ready;
        self.nmi_pending = state.nmi_pending;
        self.address_latch = state.address_latch;
        self.temp_addr = state.temp_addr;
        self.vram_addr = state.vram_addr;
        self.read_buffer = state.read_buffer;
        self.framebuffer.clone_from(&state.framebuffer);
    }

    fn read_ppudata(&mut self, mapper: &dyn Mapper) -> u8 {
        let addr = self.vram_addr & 0x3fff;
        self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment());
        if addr >= 0x3f00 {
            self.read_palette(addr)
        } else {
            let value = self.read_vram_or_chr(addr, mapper);
            let old = self.read_buffer;
            self.read_buffer = value;
            old
        }
    }

    fn write_ppudata(&mut self, value: u8, mapper: &mut dyn Mapper) {
        let addr = self.vram_addr & 0x3fff;
        self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment());
        match addr {
            0x0000..=0x1fff => {
                mapper.ppu_write(addr, value);
            }
            0x2000..=0x3eff => {
                let index = self.nametable_index(addr);
                self.vram[index] = value;
            }
            0x3f00..=0x3fff => {
                let index = Self::palette_index(addr);
                self.palette[index] = value;
            }
            _ => {}
        }
    }

    fn read_vram_or_chr(&self, addr: u16, mapper: &dyn Mapper) -> u8 {
        match addr {
            0x0000..=0x1fff => mapper.ppu_read(addr).unwrap_or(0),
            0x2000..=0x3eff => self.vram[self.nametable_index(addr)],
            _ => 0,
        }
    }

    fn read_palette(&self, addr: u16) -> u8 {
        self.palette[Self::palette_index(addr)]
    }

    fn palette_index(addr: u16) -> usize {
        let mut index = ((addr - 0x3f00) % 0x20) as usize;
        if matches!(index, 0x10 | 0x14 | 0x18 | 0x1c) {
            index -= 0x10;
        }
        index
    }

    fn nametable_index(&self, addr: u16) -> usize {
        let offset = (addr - 0x2000) % 0x1000;
        let table = offset / 0x400;
        let inner = offset % 0x400;
        let mapped_table = match self.mirroring {
            Mirroring::Vertical => table % 2,
            Mirroring::Horizontal => table / 2,
            Mirroring::FourScreen => table % 2,
        };
        (mapped_table * 0x400 + inner) as usize
    }

    fn vram_increment(&self) -> u16 {
        if self.registers[0] & 0x04 != 0 { 32 } else { 1 }
    }

    fn draw_placeholder_frame(&mut self) {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let i = (y * WIDTH + x) * 4;
                let checker = ((x / 16) + (y / 16)) % 2 == 0;
                self.framebuffer[i] = if checker { 84 } else { 116 };
                self.framebuffer[i + 1] = if y < 120 { 140 } else { 92 };
                self.framebuffer[i + 2] = if checker { 204 } else { 156 };
                self.framebuffer[i + 3] = 255;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PpuState {
    pub registers: [u8; 8],
    pub vram: Vec<u8>,
    pub palette: [u8; 0x20],
    pub oam: [u8; 0x100],
    pub oam_addr: u8,
    pub scanline: i16,
    pub dot: u16,
    pub frame_ready: bool,
    pub nmi_pending: bool,
    pub address_latch: bool,
    pub temp_addr: u16,
    pub vram_addr: u16,
    pub read_buffer: u8,
    pub framebuffer: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::mapper0::Mapper0;

    #[test]
    fn status_read_clears_vblank_and_latch() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mapper = Mapper0::new(vec![0; 0x8000], vec![0; 0x2000], true);
        ppu.registers[2] = 0x80;
        assert_eq!(ppu.cpu_read_register(0x2002, &mapper), 0x80);
        assert_eq!(ppu.registers[2] & 0x80, 0);
    }

    #[test]
    fn palette_mirrors_universal_background_entries() {
        assert_eq!(Ppu::palette_index(0x3f10), Ppu::palette_index(0x3f00));
    }

    #[test]
    fn raises_frame_ready_at_vblank() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.step(341 * 241);
        assert!(ppu.frame_ready());
        assert_eq!(ppu.take_frame().len(), FRAMEBUFFER_SIZE);
        assert!(!ppu.frame_ready());
    }
}
