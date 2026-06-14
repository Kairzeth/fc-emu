use serde::{Deserialize, Serialize};

pub mod channel;
pub mod mixer;

#[derive(Clone, Debug)]
pub struct Apu {
    registers: [u8; 0x18],
    sample_clock: f32,
    frame_clock: f32,
    samples: Vec<f32>,
    enabled: u8,
    length_counters: [u8; 4],
    pulse_phase: [f32; 2],
    triangle_phase: f32,
    noise_phase: f32,
    noise_shift: u16,
    dmc_output: f32,
}

const CPU_CLOCK_HZ: f32 = 1_789_773.0;
const SAMPLE_RATE: f32 = 44_100.0;
const CYCLES_PER_SAMPLE: f32 = CPU_CLOCK_HZ / SAMPLE_RATE;
const CYCLES_PER_QUARTER_FRAME: f32 = CPU_CLOCK_HZ / 240.0;
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];
const NOISE_PERIODS: [f32; 16] = [
    4.0, 8.0, 16.0, 32.0, 64.0, 96.0, 128.0, 160.0, 202.0, 254.0, 380.0, 508.0, 762.0, 1_016.0,
    2_034.0, 4_068.0,
];

impl Default for Apu {
    fn default() -> Self {
        Self {
            registers: [0; 0x18],
            sample_clock: 0.0,
            frame_clock: 0.0,
            samples: Vec::new(),
            enabled: 0,
            length_counters: [0; 4],
            pulse_phase: [0.0; 2],
            triangle_phase: 0.0,
            noise_phase: 0.0,
            noise_shift: 1,
            dmc_output: 64.0,
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
            for channel in 0..4 {
                if self.enabled & (1 << channel) == 0 {
                    self.length_counters[channel] = 0;
                }
            }
        }
        if addr == 0x4011 {
            self.dmc_output = f32::from(value & 0x7f);
        }
        if (0x4000..=0x4017).contains(&addr) {
            self.registers[(addr - 0x4000) as usize] = value;
        }
        match addr {
            0x4003 => self.length_counters[0] = length_counter_load(value),
            0x4007 => self.length_counters[1] = length_counter_load(value),
            0x400b => self.length_counters[2] = length_counter_load(value),
            0x400f => self.length_counters[3] = length_counter_load(value),
            _ => {}
        }
    }

    pub fn step(&mut self, cpu_cycles: u16) {
        if self.enabled == 0 {
            return;
        }

        let cpu_cycles = f32::from(cpu_cycles);
        self.frame_clock += cpu_cycles;
        while self.frame_clock >= CYCLES_PER_QUARTER_FRAME {
            self.frame_clock -= CYCLES_PER_QUARTER_FRAME;
            self.clock_length_counters();
        }

        self.sample_clock += cpu_cycles;
        while self.sample_clock >= CYCLES_PER_SAMPLE {
            self.sample_clock -= CYCLES_PER_SAMPLE;
            let sample = self.sample();
            self.samples.push(sample);
        }
    }

    pub fn drain_samples(&mut self, output: &mut Vec<f32>) {
        output.append(&mut self.samples);
    }

    pub fn snapshot(&self) -> ApuState {
        ApuState {
            registers: self.registers,
            sample_clock: self.sample_clock,
            frame_clock: self.frame_clock,
            enabled: self.enabled,
            length_counters: self.length_counters,
            pulse_phase: self.pulse_phase,
            triangle_phase: self.triangle_phase,
            noise_phase: self.noise_phase,
            noise_shift: self.noise_shift,
            dmc_output: self.dmc_output,
        }
    }

    pub fn restore(&mut self, state: &ApuState) {
        self.registers = state.registers;
        self.sample_clock = state.sample_clock;
        self.frame_clock = state.frame_clock;
        self.enabled = state.enabled;
        self.length_counters = state.length_counters;
        if self.enabled != 0 && self.length_counters == [0; 4] {
            self.rebuild_length_counters_from_registers();
        }
        self.pulse_phase = state.pulse_phase;
        self.triangle_phase = state.triangle_phase;
        self.noise_phase = state.noise_phase;
        self.noise_shift = state.noise_shift;
        self.dmc_output = state.dmc_output;
        self.samples.clear();
    }

    fn sample(&mut self) -> f32 {
        let pulse_1 = self.pulse_sample(0);
        let pulse_2 = self.pulse_sample(1);
        let triangle = self.triangle_sample();
        let noise = self.noise_sample();
        let dmc = self.dmc_sample();
        mixer::mix(pulse_1, pulse_2, triangle, noise, dmc)
    }

    fn pulse_sample(&mut self, channel: usize) -> f32 {
        let enabled_mask = 1 << channel;
        if self.enabled & enabled_mask == 0 || self.length_counters[channel] == 0 {
            return 0.0;
        }

        let base = channel * 4;
        let timer = self.timer(base + 2, base + 3);
        if timer < 8 {
            return 0.0;
        }

        let frequency = CPU_CLOCK_HZ / (16.0 * (f32::from(timer) + 1.0));
        self.pulse_phase[channel] = (self.pulse_phase[channel] + frequency / SAMPLE_RATE) % 1.0;

        let duty = match self.registers[base] >> 6 {
            0 => 0.125,
            1 => 0.25,
            2 => 0.5,
            _ => 0.75,
        };
        let volume = self.volume(base);
        if self.pulse_phase[channel] < duty {
            volume
        } else {
            0.0
        }
    }

    fn triangle_sample(&mut self) -> f32 {
        if self.enabled & 0x04 == 0 || self.length_counters[2] == 0 {
            return 0.0;
        }

        let timer = self.timer(0x0a, 0x0b);
        if timer == 0 {
            return 0.0;
        }

        let frequency = CPU_CLOCK_HZ / (32.0 * (f32::from(timer) + 1.0));
        self.triangle_phase = (self.triangle_phase + frequency / SAMPLE_RATE) % 1.0;
        let wave = if self.triangle_phase < 0.5 {
            self.triangle_phase * 4.0 - 1.0
        } else {
            3.0 - self.triangle_phase * 4.0
        };
        (wave + 1.0) * 7.5
    }

    fn noise_sample(&mut self) -> f32 {
        if self.enabled & 0x08 == 0 || self.length_counters[3] == 0 {
            return 0.0;
        }

        let period = NOISE_PERIODS[usize::from(self.registers[0x0e] & 0x0f)];
        self.noise_phase += CYCLES_PER_SAMPLE;
        while self.noise_phase >= period {
            self.noise_phase -= period;
            let tap = if self.registers[0x0e] & 0x80 == 0 {
                1
            } else {
                6
            };
            let feedback = (self.noise_shift ^ (self.noise_shift >> tap)) & 1;
            self.noise_shift = (self.noise_shift >> 1) | (feedback << 14);
        }

        let volume = self.volume(0x0c);
        if self.noise_shift & 1 == 0 {
            volume
        } else {
            0.0
        }
    }

    fn dmc_sample(&self) -> f32 {
        if self.enabled & 0x10 == 0 {
            0.0
        } else {
            self.dmc_output
        }
    }

    fn timer(&self, low_register: usize, high_register: usize) -> u16 {
        u16::from(self.registers[low_register])
            | (u16::from(self.registers[high_register] & 0x07) << 8)
    }

    fn volume(&self, control_register: usize) -> f32 {
        f32::from(self.registers[control_register] & 0x0f)
    }

    fn clock_length_counters(&mut self) {
        for (channel, counter) in self.length_counters.iter_mut().enumerate() {
            let halt_flag = match channel {
                0 => self.registers[0x00] & 0x20 != 0,
                1 => self.registers[0x04] & 0x20 != 0,
                2 => self.registers[0x08] & 0x80 != 0,
                3 => self.registers[0x0c] & 0x20 != 0,
                _ => false,
            };
            if !halt_flag && *counter > 0 {
                *counter -= 1;
            }
        }
    }

    fn rebuild_length_counters_from_registers(&mut self) {
        let high_registers = [0x03, 0x07, 0x0b, 0x0f];
        for (channel, high_register) in high_registers.into_iter().enumerate() {
            if self.enabled & (1 << channel) != 0 {
                self.length_counters[channel] = length_counter_load(self.registers[high_register]);
            }
        }
    }
}

fn length_counter_load(value: u8) -> u8 {
    LENGTH_TABLE[usize::from(value >> 3)]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApuState {
    pub registers: [u8; 0x18],
    pub sample_clock: f32,
    #[serde(default)]
    pub frame_clock: f32,
    pub enabled: u8,
    #[serde(default)]
    pub length_counters: [u8; 4],
    pub pulse_phase: [f32; 2],
    pub triangle_phase: f32,
    pub noise_phase: f32,
    pub noise_shift: u16,
    pub dmc_output: f32,
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

    #[test]
    fn enabled_pulse_generates_bounded_samples_from_timer_registers() {
        let mut apu = Apu::default();
        apu.cpu_write_register(0x4000, 0b0101_1111);
        apu.cpu_write_register(0x4002, 0xfe);
        apu.cpu_write_register(0x4003, 0x00);
        apu.cpu_write_register(0x4015, 0x01);

        for _ in 0..200 {
            apu.step(16);
        }

        let mut samples = Vec::new();
        apu.drain_samples(&mut samples);
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }

    #[test]
    fn triangle_and_noise_contribute_when_enabled() {
        let mut apu = Apu::default();
        apu.cpu_write_register(0x4008, 0x80);
        apu.cpu_write_register(0x400a, 0xff);
        apu.cpu_write_register(0x400b, 0x00);
        apu.cpu_write_register(0x400c, 0x0f);
        apu.cpu_write_register(0x400e, 0x00);
        apu.cpu_write_register(0x4015, 0x0c);

        for _ in 0..200 {
            apu.step(16);
        }

        let mut samples = Vec::new();
        apu.drain_samples(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    fn dmc_direct_load_contributes_small_bounded_output() {
        let mut apu = Apu::default();
        apu.cpu_write_register(0x4011, 0x7f);
        apu.cpu_write_register(0x4015, 0x10);

        for _ in 0..200 {
            apu.step(16);
        }

        let mut samples = Vec::new();
        apu.drain_samples(&mut samples);
        assert!(samples.iter().any(|sample| *sample > 0.0));
        assert!(samples.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }
}
