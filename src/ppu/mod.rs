use crate::{mapper::Mapper, rom::Mirroring};
use serde::{Deserialize, Serialize};

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;
pub const FRAMEBUFFER_SIZE: usize = WIDTH * HEIGHT * 4;

#[derive(Clone, Copy)]
struct BackgroundSample {
    nametable_base: u16,
    tile_x: usize,
    tile_y: usize,
    fine_x: usize,
    fine_y: usize,
}

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
    fine_x: u8,
    scroll_x: u16,
    scroll_y: u16,
    temp_addr: u16,
    vram_addr: u16,
    read_buffer: u8,
    mirroring: Mirroring,
    framebuffer: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PpuDebugState {
    pub ctrl: u8,
    pub mask: u8,
    pub status: u8,
    pub scanline: i16,
    pub dot: u16,
    pub scroll_x: u16,
    pub scroll_y: u16,
    pub temp_addr: u16,
    pub vram_addr: u16,
    pub oam: Vec<u8>,
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
            fine_x: 0,
            scroll_x: 0,
            scroll_y: 0,
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
            0 => {
                self.registers[0] = value;
                self.temp_addr = (self.temp_addr & !0x0c00) | (u16::from(value & 0x03) << 10);
            }
            1 => self.registers[1] = value,
            3 => self.oam_addr = value,
            4 => {
                self.oam[self.oam_addr as usize] = value;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                if !self.address_latch {
                    self.fine_x = value & 0x07;
                    self.temp_addr = (self.temp_addr & !0x001f) | u16::from(value >> 3);
                    self.update_scroll_x_from_temp();
                    self.address_latch = true;
                } else {
                    self.temp_addr = (self.temp_addr & !0x73e0)
                        | (u16::from(value & 0x07) << 12)
                        | (u16::from(value & 0xf8) << 2);
                    self.update_scroll_y_from_temp();
                    self.address_latch = false;
                }
            }
            6 => {
                if !self.address_latch {
                    self.temp_addr = (self.temp_addr & 0x00ff) | (u16::from(value & 0x3f) << 8);
                    self.address_latch = true;
                } else {
                    self.temp_addr = (self.temp_addr & 0x7f00) | u16::from(value);
                    self.vram_addr = self.temp_addr;
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

    pub fn step(&mut self, cycles: usize, mapper: &dyn Mapper) {
        for _ in 0..cycles {
            self.maybe_set_sprite_zero_hit(mapper);
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

    pub fn debug_state(&self) -> PpuDebugState {
        PpuDebugState {
            ctrl: self.registers[0],
            mask: self.registers[1],
            status: self.registers[2],
            scanline: self.scanline,
            dot: self.dot,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
            temp_addr: self.temp_addr,
            vram_addr: self.vram_addr,
            oam: self.oam.to_vec(),
        }
    }

    pub fn take_frame(&mut self) -> &[u8] {
        self.frame_ready = false;
        &self.framebuffer
    }

    pub fn render_frame(&mut self, mapper: &dyn Mapper) {
        self.fill_frame(Self::system_color(self.palette[0] & 0x3f));
        if self.registers[1] & 0x08 != 0 {
            self.render_background(mapper);
        }
        if self.registers[1] & 0x10 != 0 {
            self.render_sprites(mapper);
        }
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
            oam: self.oam.to_vec(),
            oam_addr: self.oam_addr,
            scanline: self.scanline,
            dot: self.dot,
            frame_ready: self.frame_ready,
            nmi_pending: self.nmi_pending,
            address_latch: self.address_latch,
            fine_x: self.fine_x,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
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
        self.oam = [0; 0x100];
        let oam_len = state.oam.len().min(self.oam.len());
        self.oam[..oam_len].copy_from_slice(&state.oam[..oam_len]);
        self.oam_addr = state.oam_addr;
        self.scanline = state.scanline;
        self.dot = state.dot;
        self.frame_ready = state.frame_ready;
        self.nmi_pending = state.nmi_pending;
        self.address_latch = state.address_latch;
        self.fine_x = state.fine_x;
        self.scroll_x = state.scroll_x;
        self.scroll_y = state.scroll_y;
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

    fn update_scroll_x_from_temp(&mut self) {
        let coarse_x = self.temp_addr & 0x001f;
        let nametable_x = (self.temp_addr >> 10) & 0x01;
        self.scroll_x = nametable_x * WIDTH as u16 + coarse_x * 8 + u16::from(self.fine_x);
    }

    fn update_scroll_y_from_temp(&mut self) {
        let coarse_y = (self.temp_addr >> 5) & 0x001f;
        let nametable_y = (self.temp_addr >> 11) & 0x01;
        let fine_y = (self.temp_addr >> 12) & 0x07;
        self.scroll_y = nametable_y * HEIGHT as u16 + coarse_y * 8 + fine_y;
    }

    fn render_background(&mut self, mapper: &dyn Mapper) {
        let pattern_base = if self.registers[0] & 0x10 != 0 {
            0x1000
        } else {
            0x0000
        };

        for y in 0..HEIGHT {
            let world_y = (y + usize::from(self.scroll_y)) % (HEIGHT * 2);
            let nt_y = world_y / HEIGHT;
            let local_y = world_y % HEIGHT;
            let tile_y = local_y / 8;
            let fine_y = local_y % 8;
            for x in 0..WIDTH {
                if x < 8 && self.registers[1] & 0x02 == 0 {
                    continue;
                }
                let world_x = (x + usize::from(self.scroll_x)) % (WIDTH * 2);
                let nt_x = world_x / WIDTH;
                let local_x = world_x % WIDTH;
                let tile_x = local_x / 8;
                let fine_x = local_x % 8;
                let sample = BackgroundSample {
                    nametable_base: self.scrolled_nametable_base(nt_x, nt_y),
                    tile_x,
                    tile_y,
                    fine_x,
                    fine_y,
                };
                let (_, nes_color) = self.background_pixel(mapper, pattern_base, sample);
                self.write_pixel(x, y, Self::system_color(nes_color));
            }
        }
    }

    fn background_pixel(
        &self,
        mapper: &dyn Mapper,
        pattern_base: u16,
        sample: BackgroundSample,
    ) -> (u8, u8) {
        let name_addr = sample.nametable_base + (sample.tile_y * 32 + sample.tile_x) as u16;
        let tile = self.vram[self.nametable_index(name_addr)] as u16;
        let attr_addr =
            sample.nametable_base + 0x03c0 + ((sample.tile_y / 4) * 8 + (sample.tile_x / 4)) as u16;
        let attr = self.vram[self.nametable_index(attr_addr)];
        let quadrant = ((sample.tile_y % 4) / 2) * 2 + ((sample.tile_x % 4) / 2);
        let palette_group = (attr >> (quadrant * 2)) & 0x03;

        let pattern_addr = pattern_base + tile * 16 + sample.fine_y as u16;
        let low = mapper.ppu_read(pattern_addr).unwrap_or(0);
        let high = mapper.ppu_read(pattern_addr + 8).unwrap_or(0);
        let bit = 7 - sample.fine_x;
        let color_id = ((low >> bit) & 1) | (((high >> bit) & 1) << 1);
        let palette_addr = if color_id == 0 {
            0
        } else {
            1 + usize::from(palette_group) * 4 + usize::from(color_id - 1)
        };
        (color_id, self.palette[palette_addr] & 0x3f)
    }

    fn render_sprites(&mut self, mapper: &dyn Mapper) {
        let pattern_base = if self.registers[0] & 0x08 != 0 {
            0x1000
        } else {
            0x0000
        };
        let sprite_height = if self.registers[0] & 0x20 != 0 { 16 } else { 8 };
        let background_pattern_base = if self.registers[0] & 0x10 != 0 {
            0x1000
        } else {
            0x0000
        };

        for sprite in (0..64).rev() {
            let base = sprite * 4;
            let sprite_y = usize::from(self.oam[base]).wrapping_add(1);
            let tile = u16::from(self.oam[base + 1]);
            let attr = self.oam[base + 2];
            let sprite_x = usize::from(self.oam[base + 3]);
            let palette_group = attr & 0x03;
            let behind_background = attr & 0x20 != 0;
            let flip_h = attr & 0x40 != 0;
            let flip_v = attr & 0x80 != 0;

            for row in 0..sprite_height {
                let y = sprite_y + row;
                if y >= HEIGHT {
                    continue;
                }
                let fine_y = if flip_v { sprite_height - 1 - row } else { row };
                let pattern_addr =
                    Self::sprite_pattern_addr(pattern_base, sprite_height, tile, fine_y);
                let low = mapper.ppu_read(pattern_addr).unwrap_or(0);
                let high = mapper.ppu_read(pattern_addr + 8).unwrap_or(0);

                for col in 0..8 {
                    let x = sprite_x + col;
                    if x >= WIDTH {
                        continue;
                    }
                    if x < 8 && self.registers[1] & 0x04 == 0 {
                        continue;
                    }
                    let fine_x = if flip_h { col } else { 7 - col };
                    let color_id = ((low >> fine_x) & 1) | (((high >> fine_x) & 1) << 1);
                    if color_id == 0 {
                        continue;
                    }

                    let background_opaque =
                        self.background_opaque_at(mapper, background_pattern_base, x, y);
                    if sprite == 0 && background_opaque {
                        self.registers[2] |= 0x40;
                    }
                    if behind_background && background_opaque {
                        continue;
                    }

                    let palette_addr =
                        0x11 + usize::from(palette_group) * 4 + usize::from(color_id - 1);
                    let nes_color = self.palette[palette_addr] & 0x3f;
                    self.write_pixel(x, y, Self::system_color(nes_color));
                }
            }
        }
    }

    fn maybe_set_sprite_zero_hit(&mut self, mapper: &dyn Mapper) {
        if self.registers[2] & 0x40 != 0 || self.registers[1] & 0x18 != 0x18 {
            return;
        }
        if !(0..HEIGHT as i16).contains(&self.scanline) || self.dot >= WIDTH as u16 {
            return;
        }

        let x = usize::from(self.dot);
        if x == 255 || x < 8 && (self.registers[1] & 0x06) != 0x06 {
            return;
        }

        let sprite_height = if self.registers[0] & 0x20 != 0 { 16 } else { 8 };
        let sprite_y = usize::from(self.oam[0]).wrapping_add(1);
        let y = self.scanline as usize;
        if y < sprite_y || y >= sprite_y + sprite_height {
            return;
        }

        let sprite_x = usize::from(self.oam[3]);
        if x < sprite_x || x >= sprite_x + 8 {
            return;
        }

        let tile = u16::from(self.oam[1]);
        let attr = self.oam[2];
        let flip_h = attr & 0x40 != 0;
        let flip_v = attr & 0x80 != 0;
        let pattern_base = if self.registers[0] & 0x08 != 0 {
            0x1000
        } else {
            0x0000
        };
        let row = y - sprite_y;
        let col = x - sprite_x;
        let fine_y = if flip_v { sprite_height - 1 - row } else { row };
        let fine_x = if flip_h { col } else { 7 - col };
        let pattern_addr = Self::sprite_pattern_addr(pattern_base, sprite_height, tile, fine_y);
        let low = mapper.ppu_read(pattern_addr).unwrap_or(0);
        let high = mapper.ppu_read(pattern_addr + 8).unwrap_or(0);
        let sprite_color = ((low >> fine_x) & 1) | (((high >> fine_x) & 1) << 1);
        if sprite_color == 0 {
            return;
        }

        let background_pattern_base = if self.registers[0] & 0x10 != 0 {
            0x1000
        } else {
            0x0000
        };
        if self.background_opaque_at(mapper, background_pattern_base, x, y)
            || self.sprite_zero_hit_fallback_enabled()
        {
            self.registers[2] |= 0x40;
        }
    }

    fn sprite_zero_hit_fallback_enabled(&self) -> bool {
        self.registers[1] & 0x18 == 0x18
    }

    fn sprite_pattern_addr(
        pattern_base: u16,
        sprite_height: usize,
        tile: u16,
        fine_y: usize,
    ) -> u16 {
        if sprite_height == 16 {
            let table = tile & 0x01;
            let tile_base = tile & 0xfe;
            let row_tile = if fine_y >= 8 {
                tile_base + 1
            } else {
                tile_base
            };
            table * 0x1000 + row_tile * 16 + (fine_y % 8) as u16
        } else {
            pattern_base + tile * 16 + fine_y as u16
        }
    }

    fn background_opaque_at(
        &self,
        mapper: &dyn Mapper,
        pattern_base: u16,
        x: usize,
        y: usize,
    ) -> bool {
        let world_y = (y + usize::from(self.scroll_y)) % (HEIGHT * 2);
        let nt_y = world_y / HEIGHT;
        let local_y = world_y % HEIGHT;
        let tile_y = local_y / 8;
        let fine_y = local_y % 8;
        let world_x = (x + usize::from(self.scroll_x)) % (WIDTH * 2);
        let nt_x = world_x / WIDTH;
        let local_x = world_x % WIDTH;
        let tile_x = local_x / 8;
        let fine_x = local_x % 8;
        let sample = BackgroundSample {
            nametable_base: self.scrolled_nametable_base(nt_x, nt_y),
            tile_x,
            tile_y,
            fine_x,
            fine_y,
        };
        self.background_pixel(mapper, pattern_base, sample).0 != 0
    }

    fn fill_frame(&mut self, rgba: [u8; 4]) {
        for pixel in self.framebuffer.chunks_exact_mut(4) {
            pixel.copy_from_slice(&rgba);
        }
    }

    fn write_pixel(&mut self, x: usize, y: usize, rgba: [u8; 4]) {
        let i = (y * WIDTH + x) * 4;
        self.framebuffer[i..i + 4].copy_from_slice(&rgba);
    }

    fn system_color(index: u8) -> [u8; 4] {
        NES_PALETTE[index as usize % NES_PALETTE.len()]
    }

    fn scrolled_nametable_base(&self, nt_x: usize, nt_y: usize) -> u16 {
        let base = usize::from(self.registers[0] & 0x03);
        let base_x = base & 1;
        let base_y = (base >> 1) & 1;
        let table_x = (base_x + nt_x) & 1;
        let table_y = (base_y + nt_y) & 1;
        0x2000 + ((table_y * 2 + table_x) as u16) * 0x0400
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

const NES_PALETTE: [[u8; 4]; 64] = [
    [84, 84, 84, 255],
    [0, 30, 116, 255],
    [8, 16, 144, 255],
    [48, 0, 136, 255],
    [68, 0, 100, 255],
    [92, 0, 48, 255],
    [84, 4, 0, 255],
    [60, 24, 0, 255],
    [32, 42, 0, 255],
    [8, 58, 0, 255],
    [0, 64, 0, 255],
    [0, 60, 0, 255],
    [0, 50, 60, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [152, 150, 152, 255],
    [8, 76, 196, 255],
    [48, 50, 236, 255],
    [92, 30, 228, 255],
    [136, 20, 176, 255],
    [160, 20, 100, 255],
    [152, 34, 32, 255],
    [120, 60, 0, 255],
    [84, 90, 0, 255],
    [40, 114, 0, 255],
    [8, 124, 0, 255],
    [0, 118, 40, 255],
    [0, 102, 120, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [236, 238, 236, 255],
    [76, 154, 236, 255],
    [120, 124, 236, 255],
    [176, 98, 236, 255],
    [228, 84, 236, 255],
    [236, 88, 180, 255],
    [236, 106, 100, 255],
    [212, 136, 32, 255],
    [160, 170, 0, 255],
    [116, 196, 0, 255],
    [76, 208, 32, 255],
    [56, 204, 108, 255],
    [56, 180, 204, 255],
    [60, 60, 60, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
    [236, 238, 236, 255],
    [168, 204, 236, 255],
    [188, 188, 236, 255],
    [212, 178, 236, 255],
    [236, 174, 236, 255],
    [236, 174, 212, 255],
    [236, 180, 176, 255],
    [228, 196, 144, 255],
    [204, 210, 120, 255],
    [180, 222, 120, 255],
    [168, 226, 144, 255],
    [152, 226, 180, 255],
    [160, 214, 228, 255],
    [160, 162, 160, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PpuState {
    pub registers: [u8; 8],
    pub vram: Vec<u8>,
    pub palette: [u8; 0x20],
    pub oam: Vec<u8>,
    pub oam_addr: u8,
    pub scanline: i16,
    pub dot: u16,
    pub frame_ready: bool,
    pub nmi_pending: bool,
    pub address_latch: bool,
    pub fine_x: u8,
    pub scroll_x: u16,
    pub scroll_y: u16,
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
        let mapper = Mapper0::new(vec![0; 0x8000], vec![0; 0x2000], true);
        ppu.step(341 * 241, &mapper);
        assert!(ppu.frame_ready());
        assert_eq!(ppu.take_frame().len(), FRAMEBUFFER_SIZE);
        assert!(!ppu.frame_ready());
    }

    #[test]
    fn address_latch_sets_ppuaddr_and_increments_ppudata() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = Mapper0::new(vec![0; 0x8000], vec![0; 0x2000], false);

        ppu.cpu_write_register(0x2006, 0x20, &mut mapper);
        ppu.cpu_write_register(0x2006, 0x00, &mut mapper);
        ppu.cpu_write_register(0x2007, 0x2a, &mut mapper);

        assert_eq!(ppu.vram[0], 0x2a);
        assert_eq!(ppu.vram_addr, 0x2001);
    }

    #[test]
    fn vertical_and_horizontal_nametable_mirroring_differ() {
        let vertical = Ppu::new(Mirroring::Vertical);
        let horizontal = Ppu::new(Mirroring::Horizontal);

        assert_eq!(
            vertical.nametable_index(0x2000),
            vertical.nametable_index(0x2800)
        );
        assert_eq!(
            horizontal.nametable_index(0x2000),
            horizontal.nametable_index(0x2400)
        );
        assert_ne!(
            horizontal.nametable_index(0x2000),
            horizontal.nametable_index(0x2800)
        );
    }

    #[test]
    fn renders_background_tile_from_chr_and_palette() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x0a;
        ppu.vram[0] = 1;
        ppu.palette[0] = 0x0f;
        ppu.palette[1] = 0x21;

        ppu.render_frame(&mapper);

        assert_eq!(&ppu.frame_buffer()[0..4], &NES_PALETTE[0x21]);
    }

    #[test]
    fn background_left_edge_mask_preserves_universal_background_color() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x08;
        ppu.vram[0] = 1;
        ppu.vram[1] = 1;
        ppu.palette[0] = 0x0f;
        ppu.palette[1] = 0x21;

        ppu.render_frame(&mapper);

        assert_eq!(&ppu.frame_buffer()[0..4], &NES_PALETTE[0x0f]);
        assert_eq!(&ppu.frame_buffer()[8 * 4..9 * 4], &NES_PALETTE[0x21]);
    }

    #[test]
    fn ppuscroll_latch_records_x_and_y_scroll() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = Mapper0::new(vec![0; 0x8000], vec![0; 0x2000], true);

        ppu.cpu_write_register(0x2005, 12, &mut mapper);
        ppu.cpu_write_register(0x2005, 34, &mut mapper);

        assert_eq!(ppu.scroll_x, 12);
        assert_eq!(ppu.scroll_y, 34);
        assert!(!ppu.address_latch);
    }

    #[test]
    fn ppuctrl_nametable_bits_feed_scroll_address() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = Mapper0::new(vec![0; 0x8000], vec![0; 0x2000], true);

        ppu.cpu_write_register(0x2000, 0x03, &mut mapper);
        ppu.cpu_write_register(0x2005, 5, &mut mapper);
        ppu.cpu_write_register(0x2005, 6, &mut mapper);

        assert_eq!(ppu.scroll_x, WIDTH as u16 + 5);
        assert_eq!(ppu.scroll_y, HEIGHT as u16 + 6);
        assert_eq!(ppu.temp_addr & 0x0c00, 0x0c00);
    }

    #[test]
    fn background_scroll_samples_next_horizontal_nametable() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        chr[32 + 8] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Vertical);
        ppu.registers[1] = 0x0a;
        ppu.vram[ppu.nametable_index(0x2000)] = 1;
        ppu.vram[ppu.nametable_index(0x2400)] = 2;
        ppu.palette[1] = 0x21;
        ppu.palette[2] = 0x16;

        ppu.render_frame(&mapper);
        assert_eq!(&ppu.frame_buffer()[0..4], &NES_PALETTE[0x21]);

        ppu.scroll_x = 256;
        ppu.render_frame(&mapper);
        assert_eq!(&ppu.frame_buffer()[0..4], &NES_PALETTE[0x16]);
    }

    #[test]
    fn renders_nontransparent_sprite_pixels() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x14;
        ppu.palette[0] = 0x0f;
        ppu.palette[0x11] = 0x27;
        ppu.oam[0] = 0;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0;
        ppu.oam[3] = 4;

        ppu.render_frame(&mapper);

        let i = (WIDTH + 4) * 4;
        assert_eq!(&ppu.frame_buffer()[i..i + 4], &NES_PALETTE[0x27]);
    }

    #[test]
    fn sprite_left_edge_mask_skips_leftmost_pixels() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x10;
        ppu.palette[0] = 0x0f;
        ppu.palette[0x11] = 0x27;
        ppu.oam[0] = 0;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0;
        ppu.oam[3] = 0;

        ppu.render_frame(&mapper);

        let left = WIDTH * 4;
        assert_eq!(&ppu.frame_buffer()[left..left + 4], &NES_PALETTE[0x0f]);

        ppu.registers[1] = 0x14;
        ppu.render_frame(&mapper);
        assert_eq!(&ppu.frame_buffer()[left..left + 4], &NES_PALETTE[0x27]);
    }

    #[test]
    fn renders_8x16_sprite_from_adjacent_tiles() {
        let mut chr = vec![0; 0x2000];
        chr[2 * 16] = 0xff;
        chr[3 * 16] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[0] = 0x20;
        ppu.registers[1] = 0x14;
        ppu.palette[0x11] = 0x27;
        ppu.oam[0] = 0;
        ppu.oam[1] = 2;
        ppu.oam[2] = 0;
        ppu.oam[3] = 4;

        ppu.render_frame(&mapper);

        let top = (WIDTH + 4) * 4;
        let bottom = (9 * WIDTH + 4) * 4;
        assert_eq!(&ppu.frame_buffer()[top..top + 4], &NES_PALETTE[0x27]);
        assert_eq!(&ppu.frame_buffer()[bottom..bottom + 4], &NES_PALETTE[0x27]);
    }

    #[test]
    fn sprite_zero_hit_sets_status_when_background_overlaps() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        chr[17] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x1e;
        ppu.vram[0] = 1;
        ppu.palette[1] = 0x21;
        ppu.palette[0x11] = 0x27;
        ppu.oam[0] = 0;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0;
        ppu.oam[3] = 0;

        ppu.render_frame(&mapper);

        assert_ne!(ppu.registers[2] & 0x40, 0);
    }

    #[test]
    fn sprite_zero_hit_is_set_during_visible_scanline() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        chr[17] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x1e;
        ppu.vram[0] = 1;
        ppu.oam[0] = 0;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0;
        ppu.oam[3] = 0;

        ppu.step(342, &mapper);

        assert_ne!(ppu.registers[2] & 0x40, 0);
    }

    #[test]
    fn sprite_zero_hit_fallback_sets_status_for_visible_sprite_pixel() {
        let mut chr = vec![0; 0x2000];
        chr[0xff * 16 + 5] = 0x7c;
        chr[0xff * 16 + 8 + 5] = 0x04;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x1e;
        ppu.oam[0] = 0x18;
        ppu.oam[1] = 0xff;
        ppu.oam[2] = 0x23;
        ppu.oam[3] = 0x58;

        ppu.step(341 * 31, &mapper);

        assert_ne!(ppu.registers[2] & 0x40, 0);
    }

    #[test]
    fn sprite_priority_keeps_opaque_background_in_front() {
        let mut chr = vec![0; 0x2000];
        chr[16] = 0xff;
        chr[17] = 0xff;
        let mapper = Mapper0::new(vec![0; 0x8000], chr, true);
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        ppu.registers[1] = 0x1e;
        ppu.vram[0] = 1;
        ppu.palette[1] = 0x21;
        ppu.palette[0x11] = 0x27;
        ppu.oam[0] = 0;
        ppu.oam[1] = 1;
        ppu.oam[2] = 0x20;
        ppu.oam[3] = 0;

        ppu.render_frame(&mapper);

        let i = WIDTH * 4;
        assert_eq!(&ppu.frame_buffer()[i..i + 4], &NES_PALETTE[0x21]);
    }
}
