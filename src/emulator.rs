use crate::{
    apu::Apu,
    bus::Bus,
    cpu::Cpu,
    cpu::CpuState,
    input::{Button, Controller},
    mapper::from_rom,
    ppu::{Ppu, PpuDebugState},
    rom::Rom,
    save_state::{SAVE_VERSION, SaveState},
};

pub struct Emulator {
    cpu: Cpu,
    bus: Bus,
    rom_name: String,
}

#[derive(Clone, Debug)]
pub struct EmulatorDebugState {
    pub cpu: CpuState,
    pub ppu: PpuDebugState,
    pub cpu_ram: Vec<u8>,
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
        let mut saw_not_ready = !self.bus.frame_ready();
        let mut elapsed_cpu_cycles = 0usize;
        while elapsed_cpu_cycles < 45_000 {
            let cycles = self.cpu.step(&mut self.bus);
            elapsed_cpu_cycles += usize::from(cycles);
            self.bus.step(cycles);
            if self.bus.poll_nmi() {
                self.cpu.request_nmi();
            }
            if !self.bus.frame_ready() {
                saw_not_ready = true;
            } else if saw_not_ready {
                self.bus.render_frame();
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

    pub fn cpu_state(&self) -> CpuState {
        self.cpu.snapshot()
    }

    pub fn debug_state(&self) -> EmulatorDebugState {
        EmulatorDebugState {
            cpu: self.cpu.snapshot(),
            ppu: self.bus.ppu_debug_state(),
            cpu_ram: self.bus.cpu_ram().to_vec(),
        }
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
        if !default_rom_available() {
            return;
        }
        let rom = Rom::from_path(crate::DEFAULT_ROM_PATH).unwrap();
        let mut emulator =
            Emulator::new(rom, "Super Mario Bros. (Japan, USA).nes".to_string()).unwrap();
        emulator.step_frame();
        assert_eq!(emulator.frame_buffer().len(), crate::ppu::FRAMEBUFFER_SIZE);
        assert!(!emulator.cpu_state().stopped);
    }

    #[test]
    fn bundled_target_rom_runs_multiple_frames_without_stopping() {
        if !default_rom_available() {
            return;
        }
        let rom = Rom::from_path(crate::DEFAULT_ROM_PATH).unwrap();
        let mut emulator =
            Emulator::new(rom, "Super Mario Bros. (Japan, USA).nes".to_string()).unwrap();

        for _ in 0..120 {
            emulator.step_frame();
            assert!(!emulator.cpu_state().stopped);
        }

        let frame = emulator.frame_buffer();
        assert_eq!(frame.len(), crate::ppu::FRAMEBUFFER_SIZE);
        assert!(unique_colors(frame) > 8);
    }

    #[test]
    fn bundled_target_rom_accepts_start_input_and_keeps_running() {
        if !default_rom_available() {
            return;
        }
        let rom = Rom::from_path(crate::DEFAULT_ROM_PATH).unwrap();
        let mut emulator =
            Emulator::new(rom, "Super Mario Bros. (Japan, USA).nes".to_string()).unwrap();

        for _ in 0..90 {
            emulator.step_frame();
        }
        emulator.set_button(Button::Start, true);
        for _ in 0..8 {
            emulator.step_frame();
        }
        emulator.set_button(Button::Start, false);
        for _ in 0..120 {
            emulator.step_frame();
            assert!(!emulator.cpu_state().stopped);
        }

        assert!(unique_colors(emulator.frame_buffer()) > 4);
    }

    #[test]
    fn bundled_target_rom_framebuffer_updates_after_start() {
        if !default_rom_available() {
            return;
        }
        let rom = Rom::from_path(crate::DEFAULT_ROM_PATH).unwrap();
        let mut emulator =
            Emulator::new(rom, "Super Mario Bros. (Japan, USA).nes".to_string()).unwrap();

        for _ in 0..90 {
            emulator.step_frame();
        }
        emulator.set_button(Button::Start, true);
        for _ in 0..8 {
            emulator.step_frame();
        }
        emulator.set_button(Button::Start, false);
        for _ in 0..90 {
            emulator.step_frame();
        }
        let before = emulator.frame_buffer().to_vec();

        emulator.set_button(Button::Right, true);
        emulator.set_button(Button::A, true);
        for _ in 0..60 {
            emulator.step_frame();
        }

        assert_ne!(emulator.frame_buffer(), before.as_slice());
        assert!(!emulator.cpu_state().stopped);
    }

    #[test]
    fn bundled_target_rom_uses_only_dmc_direct_load_on_start_path() {
        if !default_rom_available() {
            return;
        }
        let rom = Rom::from_path(crate::DEFAULT_ROM_PATH).unwrap();
        let mut emulator =
            Emulator::new(rom, "Super Mario Bros. (Japan, USA).nes".to_string()).unwrap();

        for _ in 0..90 {
            emulator.step_frame();
        }
        emulator.set_button(Button::Start, true);
        for _ in 0..8 {
            emulator.step_frame();
        }
        emulator.set_button(Button::Start, false);
        for _ in 0..240 {
            emulator.step_frame();
        }

        let state = emulator.save_state();
        assert_eq!(state.apu.enabled & 0x10, 0);
        assert_eq!(state.apu.registers[0x10], 0);
        assert_ne!(state.apu.registers[0x11], 0);
        assert_eq!(&state.apu.registers[0x12..=0x13], &[0, 0]);
    }

    fn unique_colors(frame: &[u8]) -> usize {
        let mut colors: Vec<[u8; 4]> = Vec::new();
        for pixel in frame.chunks_exact(4) {
            let color = [pixel[0], pixel[1], pixel[2], pixel[3]];
            if !colors.contains(&color) {
                colors.push(color);
            }
        }
        colors.len()
    }

    fn default_rom_available() -> bool {
        std::path::Path::new(crate::DEFAULT_ROM_PATH).exists()
    }
}
