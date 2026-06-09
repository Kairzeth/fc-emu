use crate::{
    apu::{Apu, ApuState},
    input::Controller,
    mapper::Mapper,
    ppu::{Ppu, PpuState},
};
use serde::{Deserialize, Serialize};

pub const CPU_RAM_SIZE: usize = 0x0800;
pub const OAM_DMA_SIZE: usize = 256;

pub trait MapperBusDevice {
    fn cpu_read(&mut self, addr: u16) -> Option<u8>;
    fn cpu_write(&mut self, addr: u16, value: u8) -> bool;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct OamDmaState {
    pub last_page: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusState {
    pub cpu_ram: Vec<u8>,
    pub controller1: Controller,
    pub oam_dma: OamDmaState,
}

pub struct Bus {
    cpu_ram: [u8; CPU_RAM_SIZE],
    ppu: Ppu,
    apu: Apu,
    controller1: Controller,
    mapper: Box<dyn Mapper>,
    oam_dma: OamDmaState,
}

impl Bus {
    pub fn new(ppu: Ppu, apu: Apu, controller1: Controller, mapper: Box<dyn Mapper>) -> Self {
        Self {
            cpu_ram: [0; CPU_RAM_SIZE],
            ppu,
            apu,
            controller1,
            mapper,
            oam_dma: OamDmaState::default(),
        }
    }

    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.cpu_ram[ram_index(addr)],
            0x2000..=0x3FFF => self
                .ppu
                .cpu_read_register(ppu_register_addr(addr), self.mapper.as_ref()),
            0x4015 => self.apu.cpu_read_register(addr),
            0x4016 => self.controller1.read_4016(),
            0x4017 => 0,
            0x4020..=0xFFFF => self.mapper.cpu_read(addr).unwrap_or(0),
            _ => 0,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.cpu_ram[ram_index(addr)] = value,
            0x2000..=0x3FFF => {
                self.ppu
                    .cpu_write_register(ppu_register_addr(addr), value, self.mapper.as_mut())
            }
            0x4000..=0x4013 => self.apu.cpu_write_register(addr, value),
            0x4014 => self.write_oam_dma(value),
            0x4015 => self.apu.cpu_write_register(addr, value),
            0x4016 => self.controller1.write_strobe(value),
            0x4017 => self.apu.cpu_write_register(addr, value),
            0x4020..=0xFFFF => {
                self.mapper.cpu_write(addr, value);
            }
            _ => {}
        }
    }

    pub fn step(&mut self, cpu_cycles: u8) {
        self.ppu
            .step(usize::from(cpu_cycles) * 3, self.mapper.as_ref());
        self.apu.step(cpu_cycles);
    }

    pub fn render_frame(&mut self) {
        self.ppu.render_frame(self.mapper.as_ref());
    }

    pub fn poll_nmi(&mut self) -> bool {
        self.ppu.poll_nmi()
    }

    pub fn frame_ready(&self) -> bool {
        self.ppu.frame_ready()
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.ppu.frame_buffer()
    }

    pub fn drain_audio_samples(&mut self, output: &mut Vec<f32>) {
        self.apu.drain_samples(output);
    }

    pub fn controller_mut(&mut self) -> &mut Controller {
        &mut self.controller1
    }

    pub fn cpu_ram(&self) -> &[u8; CPU_RAM_SIZE] {
        &self.cpu_ram
    }

    pub fn oam_dma_state(&self) -> OamDmaState {
        self.oam_dma
    }

    pub fn snapshot(&self) -> BusState {
        BusState {
            cpu_ram: self.cpu_ram.to_vec(),
            controller1: self.controller1.clone(),
            oam_dma: self.oam_dma,
        }
    }

    pub fn ppu_snapshot(&self) -> PpuState {
        self.ppu.snapshot()
    }

    pub fn apu_snapshot(&self) -> ApuState {
        self.apu.snapshot()
    }

    pub fn restore(&mut self, bus: &BusState, ppu: &PpuState, apu: &ApuState) {
        self.cpu_ram = [0; CPU_RAM_SIZE];
        let len = bus.cpu_ram.len().min(CPU_RAM_SIZE);
        self.cpu_ram[..len].copy_from_slice(&bus.cpu_ram[..len]);
        self.controller1 = bus.controller1.clone();
        self.oam_dma = bus.oam_dma;
        self.ppu.restore(ppu);
        self.apu.restore(apu);
    }

    fn write_oam_dma(&mut self, page: u8) {
        let base = u16::from(page) << 8;
        let mut data = [0; OAM_DMA_SIZE];

        for (offset, byte) in data.iter_mut().enumerate() {
            *byte = self.cpu_read(base.wrapping_add(offset as u16));
        }

        self.oam_dma.last_page = Some(page);
        self.ppu.write_oam_dma(&data);
    }
}

fn ram_index(addr: u16) -> usize {
    usize::from(addr & 0x07FF)
}

fn ppu_register_addr(addr: u16) -> u16 {
    0x2000 + (addr & 0x0007)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mapper::mapper0::Mapper0, rom::Mirroring};

    fn test_bus() -> Bus {
        let mapper = Box::new(Mapper0::new(vec![0; 0x8000], vec![0; 0x2000], true));
        Bus::new(
            Ppu::new(Mirroring::Horizontal),
            Apu::default(),
            Controller::new(),
            mapper,
        )
    }

    fn bus_with_prg(prg: Vec<u8>) -> Bus {
        let mapper = Box::new(Mapper0::new(prg, vec![0; 0x2000], true));
        Bus::new(
            Ppu::new(Mirroring::Horizontal),
            Apu::default(),
            Controller::new(),
            mapper,
        )
    }

    #[test]
    fn mirrors_cpu_ram_every_2kb_until_0x1fff() {
        let mut bus = test_bus();

        bus.cpu_write(0x0002, 0x11);
        assert_eq!(bus.cpu_read(0x0002), 0x11);
        assert_eq!(bus.cpu_read(0x0802), 0x11);
        assert_eq!(bus.cpu_read(0x1002), 0x11);
        assert_eq!(bus.cpu_read(0x1802), 0x11);

        bus.cpu_write(0x1FFF, 0xFE);
        assert_eq!(bus.cpu_read(0x07FF), 0xFE);
    }

    #[test]
    fn mirrors_ppu_registers_to_canonical_addresses() {
        let mut bus = test_bus();

        bus.cpu_write(0x2003, 0x44);
        bus.cpu_write(0x2004, 0xAB);
        bus.cpu_write(0x3FFB, 0x44);
        assert_eq!(bus.cpu_read(0x3FFC), 0xAB);
    }

    #[test]
    fn forwards_mapper_addresses_from_0x4020_to_0xffff() {
        let mut prg = vec![0; 0x8000];
        prg[0] = 0x80;
        prg[0x3FFF] = 0xBF;
        prg[0x4000] = 0xC0;
        prg[0x7FFF] = 0xFF;
        let mut bus = bus_with_prg(prg);

        assert_eq!(bus.cpu_read(0x8000), 0x80);
        assert_eq!(bus.cpu_read(0xBFFF), 0xBF);
        assert_eq!(bus.cpu_read(0xC000), 0xC0);
        assert_eq!(bus.cpu_read(0xFFFF), 0xFF);
    }

    #[test]
    fn routes_controller_strobe_and_reads_controller_one() {
        let mut bus = test_bus();
        bus.controller_mut()
            .set_button(crate::input::Button::A, true);
        bus.controller_mut()
            .set_button(crate::input::Button::Start, true);

        bus.cpu_write(0x4016, 1);
        bus.cpu_write(0x4016, 0);

        assert_eq!(bus.cpu_read(0x4016), 1);
        assert_eq!(bus.cpu_read(0x4016), 0);
        assert_eq!(bus.cpu_read(0x4016), 0);
        assert_eq!(bus.cpu_read(0x4016), 1);
    }

    #[test]
    fn routes_apu_status_and_frame_counter() {
        let mut bus = test_bus();

        bus.cpu_write(0x4000, 0x30);
        bus.cpu_write(0x4015, 0x0F);
        bus.cpu_write(0x4017, 0x80);

        assert_eq!(bus.cpu_read(0x4015), 0x0F);
        assert_eq!(bus.cpu_read(0x4017), 0);
    }

    #[test]
    fn performs_oam_dma_from_cpu_page() {
        let mut bus = test_bus();

        for offset in 0..OAM_DMA_SIZE {
            bus.cpu_write(0x0200 + offset as u16, offset as u8);
        }

        bus.cpu_write(0x4014, 0x02);

        assert_eq!(bus.oam_dma_state().last_page, Some(0x02));
        bus.cpu_write(0x2003, 0x00);
        assert_eq!(bus.cpu_read(0x2004), 0x00);
        bus.cpu_write(0x2003, 0x7F);
        assert_eq!(bus.cpu_read(0x2004), 0x7F);
        bus.cpu_write(0x2003, 0xFF);
        assert_eq!(bus.cpu_read(0x2004), 0xFF);
    }

    #[test]
    fn snapshots_and_restores_ram_controller_and_devices() {
        let mut bus = test_bus();
        bus.cpu_write(0x0001, 0xAA);
        bus.controller_mut()
            .set_button(crate::input::Button::Right, true);
        let bus_state = bus.snapshot();
        let ppu_state = bus.ppu_snapshot();
        let apu_state = bus.apu_snapshot();

        bus.cpu_write(0x0001, 0x55);
        bus.controller_mut()
            .set_button(crate::input::Button::Right, false);
        bus.restore(&bus_state, &ppu_state, &apu_state);

        assert_eq!(bus.cpu_read(0x0001), 0xAA);
        bus.cpu_write(0x4016, 1);
        assert_eq!(bus.cpu_read(0x4016), 0);
    }
}
