#[derive(Clone, Debug)]
pub struct Apu {
    registers: [u8; 0x18],
    sample_clock: f32,
    samples: Vec<f32>,
    enabled: u8,
}

impl Default for Apu {
    fn default() -> Self {
        Self {
            registers: [0; 0x18],
            sample_clock: 0.0,
            samples: Vec::new(),
            enabled: 0,
        }
    }
}

impl Apu {
    pub fn cpu_read_register(&self, addr: u16) -> u8 {
        match addr {
            0x4015 => self.enabled,
            0x4000..=0x4017 => self.registers[(addr - 0x4000) as usize],
            _ => 0,
        }
    }

    pub fn cpu_write_register(&mut self, addr: u16, value: u8) {
        if addr == 0x4015 {
            self.enabled = value & 0x1f;
        }
        if (0x4000..=0x4017).contains(&addr) {
            self.registers[(addr - 0x4000) as usize] = value;
        }
    }

    pub fn step(&mut self, cpu_cycles: u8) {
        if self.enabled == 0 {
            return;
        }

        self.sample_clock += f32::from(cpu_cycles);
        while self.sample_clock >= 40.0 {
            self.sample_clock -= 40.0;
            let tone = if self.samples.len().is_multiple_of(2) {
                0.08
            } else {
                -0.08
            };
            self.samples.push(tone);
        }
    }

    pub fn drain_samples(&mut self, output: &mut Vec<f32>) {
        output.append(&mut self.samples);
    }

    pub fn snapshot(&self) -> ApuState {
        ApuState {
            registers: self.registers,
            sample_clock: self.sample_clock,
            enabled: self.enabled,
        }
    }

    pub fn restore(&mut self, state: &ApuState) {
        self.registers = state.registers;
        self.sample_clock = state.sample_clock;
        self.enabled = state.enabled;
        self.samples.clear();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApuState {
    pub registers: [u8; 0x18],
    pub sample_clock: f32,
    pub enabled: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_register_controls_enabled_channels() {
        let mut apu = Apu::default();
        apu.cpu_write_register(0x4015, 0b1_1111);
        assert_eq!(apu.cpu_read_register(0x4015), 0b1_1111);
    }

    #[test]
    fn disabled_apu_does_not_generate_samples() {
        let mut apu = Apu::default();
        apu.step(255);
        let mut samples = Vec::new();
        apu.drain_samples(&mut samples);
        assert!(samples.is_empty());
    }
}
