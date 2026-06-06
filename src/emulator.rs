use crate::{
    apu::Apu,
    bus::Bus,
    cpu::Cpu,
    input::{Button, Controller},
    mapper::from_rom,
    ppu::Ppu,
    rom::Rom,
    save_state::{SAVE_VERSION, SaveState},
};

pub struct Emulator {
    cpu: Cpu,
    bus: Bus,
    rom_name: String,
}

impl Emulator {
    pub fn new(rom: Rom, rom_name: String) -> Result<Self, String> {
        let mirroring = rom.mirroring;
        let mapper = from_rom(rom).map_err(|err| err.to_string())?;
        let mut bus = Bus::new(
            Ppu::new(mirroring),
            Apu::default(),
            Controller::default(),
            mapper,
        );
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        Ok(Self { cpu, bus, rom_name })
    }

    pub fn reset(&mut self) {
        self.cpu = Cpu::default();
        self.cpu.reset(&mut self.bus);
    }

    pub fn step_frame(&mut self) {
        let start_frame_ready = self.bus.frame_ready();
        for _ in 0..30_000 {
            let cycles = self.cpu.step(&mut self.bus);
            self.bus.step(cycles);
            if self.bus.poll_nmi() {
                self.cpu.nmi(&mut self.bus);
            }
            if self.bus.frame_ready() && !start_frame_ready {
                break;
            }
        }
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.bus.controller_mut().set_button(button, pressed);
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.bus.frame_buffer()
    }

    pub fn drain_audio_samples(&mut self, output: &mut Vec<f32>) {
        self.bus.drain_audio_samples(output);
    }

    pub fn save_state(&self) -> SaveState {
        SaveState {
            version: SAVE_VERSION,
            rom_name: self.rom_name.clone(),
            cpu: self.cpu.snapshot(),
            bus: self.bus.snapshot(),
            ppu: self.bus.ppu_snapshot(),
            apu: self.bus.apu_snapshot(),
        }
    }

    pub fn load_state(&mut self, state: &SaveState) {
        self.cpu.restore(&state.cpu);
        self.bus.restore(&state.bus, &state.ppu, &state.apu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::Rom;

    #[test]
    fn creates_emulator_from_synthetic_rom() {
        let mut emulator = Emulator::new(synthetic_rom(), "test.nes".to_string()).unwrap();
        emulator.step_frame();
        assert_eq!(emulator.frame_buffer().len(), crate::ppu::FRAMEBUFFER_SIZE);
    }

    fn synthetic_rom() -> Rom {
        let mut prg = vec![0xea; 0x8000];
        prg[0x7ffc] = 0x00;
        prg[0x7ffd] = 0x80;
        Rom {
            prg_rom: prg,
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            mirroring: crate::rom::Mirroring::Horizontal,
            prg_banks: 2,
            chr_banks: 1,
            has_trainer: false,
        }
    }

    #[test]
    fn saves_restores_and_continues() {
        let mut emulator = Emulator::new(synthetic_rom(), "test.nes".to_string()).unwrap();
        emulator.step_frame();
        let state = emulator.save_state();
        emulator.step_frame();
        emulator.load_state(&state);
        emulator.step_frame();
        assert_eq!(emulator.frame_buffer().len(), crate::ppu::FRAMEBUFFER_SIZE);
    }

    #[test]
    fn bundled_target_rom_loads_and_steps() {
        let rom = Rom::from_path(crate::DEFAULT_ROM_PATH).unwrap();
        let mut emulator =
            Emulator::new(rom, "Super Mario Bros. (Japan, USA).nes".to_string()).unwrap();
        emulator.step_frame();
        assert_eq!(emulator.frame_buffer().len(), crate::ppu::FRAMEBUFFER_SIZE);
    }
}
