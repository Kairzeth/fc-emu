use crate::bus::Bus;
use serde::{Deserialize, Serialize};

pub mod addressing;
pub mod opcode;
pub mod status;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterruptRequest {
    #[default]
    None,
    Nmi,
    Irq,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub status: u8,
    pub cycles: u64,
    interrupt_request: InterruptRequest,
    stopped: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xfd,
            status: status::IRQ_DISABLE | status::UNUSED,
            cycles: 0,
            interrupt_request: InterruptRequest::None,
            stopped: false,
        }
    }
}

impl Cpu {
    pub fn reset(&mut self, bus: &mut Bus) {
        let lo = u16::from(bus.cpu_read(0xfffc));
        let hi = u16::from(bus.cpu_read(0xfffd));
        self.pc = (hi << 8) | lo;
        self.sp = 0xfd;
        self.status = status::IRQ_DISABLE | status::UNUSED;
        self.interrupt_request = InterruptRequest::None;
        self.stopped = false;
        self.cycles += 7;
    }

    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        if let Some(cycles) = self.service_interrupt(bus) {
            return cycles;
        }

        if self.stopped {
            self.cycles += 2;
            return 2;
        }

        let opcode = self.fetch_byte(bus);
        let Some(instruction) = opcode::decode(opcode) else {
            eprintln!("unimplemented opcode ${opcode:02X} at ${:04X}", self.pc - 1);
            self.stopped = true;
            self.cycles += 2;
            return 2;
        };

        let cycles = match opcode {
            0x00 => {
                self.brk(bus);
                7
            }
            0xea => 2,
            0x18 => {
                self.status &= !status::CARRY;
                2
            }
            0x38 => {
                self.status |= status::CARRY;
                2
            }
            0x58 => {
                self.status &= !status::IRQ_DISABLE;
                2
            }
            0x78 => {
                self.status |= status::IRQ_DISABLE;
                2
            }
            0xb8 => {
                self.status &= !status::OVERFLOW;
                2
            }
            0xd8 => {
                self.status &= !status::DECIMAL;
                2
            }
            0xf8 => {
                self.status |= status::DECIMAL;
                2
            }
            0xa9 => {
                let value = self.fetch_byte(bus);
                self.a = value;
                self.update_zero_negative(self.a);
                2
            }
            0xa5 => {
                let addr = self.zero_page(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                3
            }
            0xb5 => {
                let addr = self.zero_page_x(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                4
            }
            0xad => {
                let addr = self.absolute(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                4
            }
            0xbd => {
                let addr = self.absolute_x(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                4
            }
            0xb9 => {
                let addr = self.absolute_y(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                4
            }
            0xa1 => {
                let addr = self.indexed_indirect(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                6
            }
            0xb1 => {
                let addr = self.indirect_indexed(bus);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                5
            }
            0xa2 => {
                let value = self.fetch_byte(bus);
                self.x = value;
                self.update_zero_negative(self.x);
                2
            }
            0xa6 => {
                let addr = self.zero_page(bus);
                self.x = bus.cpu_read(addr);
                self.update_zero_negative(self.x);
                3
            }
            0xb6 => {
                let addr = self.zero_page_y(bus);
                self.x = bus.cpu_read(addr);
                self.update_zero_negative(self.x);
                4
            }
            0xae => {
                let addr = self.absolute(bus);
                self.x = bus.cpu_read(addr);
                self.update_zero_negative(self.x);
                4
            }
            0xbe => {
                let addr = self.absolute_y(bus);
                self.x = bus.cpu_read(addr);
                self.update_zero_negative(self.x);
                4
            }
            0xa0 => {
                let value = self.fetch_byte(bus);
                self.y = value;
                self.update_zero_negative(self.y);
                2
            }
            0xa4 => {
                let addr = self.zero_page(bus);
                self.y = bus.cpu_read(addr);
                self.update_zero_negative(self.y);
                3
            }
            0xb4 => {
                let addr = self.zero_page_x(bus);
                self.y = bus.cpu_read(addr);
                self.update_zero_negative(self.y);
                4
            }
            0xac => {
                let addr = self.absolute(bus);
                self.y = bus.cpu_read(addr);
                self.update_zero_negative(self.y);
                4
            }
            0xbc => {
                let addr = self.absolute_x(bus);
                self.y = bus.cpu_read(addr);
                self.update_zero_negative(self.y);
                4
            }
            0x85 => {
                let addr = self.zero_page(bus);
                bus.cpu_write(addr, self.a);
                3
            }
            0x95 => {
                let addr = self.zero_page_x(bus);
                bus.cpu_write(addr, self.a);
                4
            }
            0x8d => {
                let addr = self.absolute(bus);
                bus.cpu_write(addr, self.a);
                4
            }
            0x9d => {
                let addr = self.absolute_x(bus);
                bus.cpu_write(addr, self.a);
                5
            }
            0x99 => {
                let addr = self.absolute_y(bus);
                bus.cpu_write(addr, self.a);
                5
            }
            0x81 => {
                let addr = self.indexed_indirect(bus);
                bus.cpu_write(addr, self.a);
                6
            }
            0x91 => {
                let addr = self.indirect_indexed(bus);
                bus.cpu_write(addr, self.a);
                6
            }
            0x86 => {
                let addr = self.zero_page(bus);
                bus.cpu_write(addr, self.x);
                3
            }
            0x96 => {
                let addr = self.zero_page_y(bus);
                bus.cpu_write(addr, self.x);
                4
            }
            0x8e => {
                let addr = self.absolute(bus);
                bus.cpu_write(addr, self.x);
                4
            }
            0x84 => {
                let addr = self.zero_page(bus);
                bus.cpu_write(addr, self.y);
                3
            }
            0x94 => {
                let addr = self.zero_page_x(bus);
                bus.cpu_write(addr, self.y);
                4
            }
            0x8c => {
                let addr = self.absolute(bus);
                bus.cpu_write(addr, self.y);
                4
            }
            0xaa => {
                self.x = self.a;
                self.update_zero_negative(self.x);
                2
            }
            0xa8 => {
                self.y = self.a;
                self.update_zero_negative(self.y);
                2
            }
            0x8a => {
                self.a = self.x;
                self.update_zero_negative(self.a);
                2
            }
            0x98 => {
                self.a = self.y;
                self.update_zero_negative(self.a);
                2
            }
            0xba => {
                self.x = self.sp;
                self.update_zero_negative(self.x);
                2
            }
            0x09 => {
                let value = self.fetch_byte(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                2
            }
            0x05 => {
                let value = self.read_zero_page(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                3
            }
            0x15 => {
                let value = self.read_zero_page_x(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                4
            }
            0x0d => {
                let value = self.read_absolute(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                4
            }
            0x1d => {
                let value = self.read_absolute_x(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                4
            }
            0x19 => {
                let value = self.read_absolute_y(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                4
            }
            0x01 => {
                let value = self.read_indexed_indirect(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                6
            }
            0x11 => {
                let value = self.read_indirect_indexed(bus);
                self.a |= value;
                self.update_zero_negative(self.a);
                5
            }
            0x29 => {
                let value = self.fetch_byte(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                2
            }
            0x25 => {
                let value = self.read_zero_page(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                3
            }
            0x35 => {
                let value = self.read_zero_page_x(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                4
            }
            0x2d => {
                let value = self.read_absolute(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                4
            }
            0x3d => {
                let value = self.read_absolute_x(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                4
            }
            0x39 => {
                let value = self.read_absolute_y(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                4
            }
            0x21 => {
                let value = self.read_indexed_indirect(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                6
            }
            0x31 => {
                let value = self.read_indirect_indexed(bus);
                self.a &= value;
                self.update_zero_negative(self.a);
                5
            }
            0x49 => {
                let value = self.fetch_byte(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                2
            }
            0x45 => {
                let value = self.read_zero_page(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                3
            }
            0x55 => {
                let value = self.read_zero_page_x(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                4
            }
            0x4d => {
                let value = self.read_absolute(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                4
            }
            0x5d => {
                let value = self.read_absolute_x(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                4
            }
            0x59 => {
                let value = self.read_absolute_y(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                4
            }
            0x41 => {
                let value = self.read_indexed_indirect(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                6
            }
            0x51 => {
                let value = self.read_indirect_indexed(bus);
                self.a ^= value;
                self.update_zero_negative(self.a);
                5
            }
            0x69 => {
                let value = self.fetch_byte(bus);
                self.adc(value);
                2
            }
            0x65 => {
                let value = self.read_zero_page(bus);
                self.adc(value);
                3
            }
            0x75 => {
                let value = self.read_zero_page_x(bus);
                self.adc(value);
                4
            }
            0x6d => {
                let value = self.read_absolute(bus);
                self.adc(value);
                4
            }
            0x7d => {
                let value = self.read_absolute_x(bus);
                self.adc(value);
                4
            }
            0x79 => {
                let value = self.read_absolute_y(bus);
                self.adc(value);
                4
            }
            0x61 => {
                let value = self.read_indexed_indirect(bus);
                self.adc(value);
                6
            }
            0x71 => {
                let value = self.read_indirect_indexed(bus);
                self.adc(value);
                5
            }
            0xe9 | 0xeb => {
                let value = self.fetch_byte(bus);
                self.sbc(value);
                2
            }
            0xe5 => {
                let value = self.read_zero_page(bus);
                self.sbc(value);
                3
            }
            0xf5 => {
                let value = self.read_zero_page_x(bus);
                self.sbc(value);
                4
            }
            0xed => {
                let value = self.read_absolute(bus);
                self.sbc(value);
                4
            }
            0xfd => {
                let value = self.read_absolute_x(bus);
                self.sbc(value);
                4
            }
            0xf9 => {
                let value = self.read_absolute_y(bus);
                self.sbc(value);
                4
            }
            0xe1 => {
                let value = self.read_indexed_indirect(bus);
                self.sbc(value);
                6
            }
            0xf1 => {
                let value = self.read_indirect_indexed(bus);
                self.sbc(value);
                5
            }
            0x24 => {
                let value = self.read_zero_page(bus);
                self.bit(value);
                3
            }
            0x2c => {
                let value = self.read_absolute(bus);
                self.bit(value);
                4
            }
            0xc9 => {
                let value = self.fetch_byte(bus);
                self.compare(self.a, value);
                2
            }
            0xc5 => {
                let value = self.read_zero_page(bus);
                self.compare(self.a, value);
                3
            }
            0xd5 => {
                let value = self.read_zero_page_x(bus);
                self.compare(self.a, value);
                4
            }
            0xcd => {
                let value = self.read_absolute(bus);
                self.compare(self.a, value);
                4
            }
            0xdd => {
                let value = self.read_absolute_x(bus);
                self.compare(self.a, value);
                4
            }
            0xd9 => {
                let value = self.read_absolute_y(bus);
                self.compare(self.a, value);
                4
            }
            0xc1 => {
                let value = self.read_indexed_indirect(bus);
                self.compare(self.a, value);
                6
            }
            0xd1 => {
                let value = self.read_indirect_indexed(bus);
                self.compare(self.a, value);
                5
            }
            0xe0 => {
                let value = self.fetch_byte(bus);
                self.compare(self.x, value);
                2
            }
            0xe4 => {
                let value = self.read_zero_page(bus);
                self.compare(self.x, value);
                3
            }
            0xec => {
                let addr = self.absolute(bus);
                let value = bus.cpu_read(addr);
                self.compare(self.x, value);
                4
            }
            0xc0 => {
                let value = self.fetch_byte(bus);
                self.compare(self.y, value);
                2
            }
            0xc4 => {
                let value = self.read_zero_page(bus);
                self.compare(self.y, value);
                3
            }
            0xcc => {
                let value = self.read_absolute(bus);
                self.compare(self.y, value);
                4
            }
            0xe6 => {
                let addr = self.zero_page(bus);
                self.inc_memory(bus, addr);
                5
            }
            0xf6 => {
                let addr = self.zero_page_x(bus);
                self.inc_memory(bus, addr);
                6
            }
            0xca => {
                self.x = self.x.wrapping_sub(1);
                self.update_zero_negative(self.x);
                2
            }
            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.update_zero_negative(self.y);
                2
            }
            0xe8 => {
                self.x = self.x.wrapping_add(1);
                self.update_zero_negative(self.x);
                2
            }
            0xc8 => {
                self.y = self.y.wrapping_add(1);
                self.update_zero_negative(self.y);
                2
            }
            0xee => {
                let addr = self.absolute(bus);
                self.inc_memory(bus, addr);
                6
            }
            0xfe => {
                let addr = self.absolute_x(bus);
                self.inc_memory(bus, addr);
                7
            }
            0xc6 => {
                let addr = self.zero_page(bus);
                self.dec_memory(bus, addr);
                5
            }
            0xd6 => {
                let addr = self.zero_page_x(bus);
                self.dec_memory(bus, addr);
                6
            }
            0xce => {
                let addr = self.absolute(bus);
                self.dec_memory(bus, addr);
                6
            }
            0xde => {
                let addr = self.absolute_x(bus);
                self.dec_memory(bus, addr);
                7
            }
            0x9a => {
                self.sp = self.x;
                2
            }
            0x48 => {
                self.push_byte(bus, self.a);
                3
            }
            0x68 => {
                self.a = self.pop_byte(bus);
                self.update_zero_negative(self.a);
                4
            }
            0x08 => {
                self.push_byte(bus, self.status | status::BREAK | status::UNUSED);
                3
            }
            0x28 => {
                self.status = (self.pop_byte(bus) | status::UNUSED) & !status::BREAK;
                4
            }
            0x0a => {
                self.a = self.asl_value(self.a);
                2
            }
            0x06 => {
                let addr = self.zero_page(bus);
                self.shift_memory(bus, addr, Cpu::asl_value);
                5
            }
            0x16 => {
                let addr = self.zero_page_x(bus);
                self.shift_memory(bus, addr, Cpu::asl_value);
                6
            }
            0x0e => {
                let addr = self.absolute(bus);
                self.shift_memory(bus, addr, Cpu::asl_value);
                6
            }
            0x1e => {
                let addr = self.absolute_x(bus);
                self.shift_memory(bus, addr, Cpu::asl_value);
                7
            }
            0x4a => {
                self.a = self.lsr_value(self.a);
                2
            }
            0x46 => {
                let addr = self.zero_page(bus);
                self.shift_memory(bus, addr, Cpu::lsr_value);
                5
            }
            0x56 => {
                let addr = self.zero_page_x(bus);
                self.shift_memory(bus, addr, Cpu::lsr_value);
                6
            }
            0x4e => {
                let addr = self.absolute(bus);
                self.shift_memory(bus, addr, Cpu::lsr_value);
                6
            }
            0x5e => {
                let addr = self.absolute_x(bus);
                self.shift_memory(bus, addr, Cpu::lsr_value);
                7
            }
            0x2a => {
                self.a = self.rol_value(self.a);
                2
            }
            0x26 => {
                let addr = self.zero_page(bus);
                self.shift_memory(bus, addr, Cpu::rol_value);
                5
            }
            0x36 => {
                let addr = self.zero_page_x(bus);
                self.shift_memory(bus, addr, Cpu::rol_value);
                6
            }
            0x2e => {
                let addr = self.absolute(bus);
                self.shift_memory(bus, addr, Cpu::rol_value);
                6
            }
            0x3e => {
                let addr = self.absolute_x(bus);
                self.shift_memory(bus, addr, Cpu::rol_value);
                7
            }
            0x6a => {
                self.a = self.ror_value(self.a);
                2
            }
            0x66 => {
                let addr = self.zero_page(bus);
                self.shift_memory(bus, addr, Cpu::ror_value);
                5
            }
            0x76 => {
                let addr = self.zero_page_x(bus);
                self.shift_memory(bus, addr, Cpu::ror_value);
                6
            }
            0x6e => {
                let addr = self.absolute(bus);
                self.shift_memory(bus, addr, Cpu::ror_value);
                6
            }
            0x7e => {
                let addr = self.absolute_x(bus);
                self.shift_memory(bus, addr, Cpu::ror_value);
                7
            }
            0x4c => {
                self.pc = self.absolute(bus);
                3
            }
            0x6c => {
                let ptr = self.absolute(bus);
                self.pc = self.read_indirect_jmp(bus, ptr);
                5
            }
            0x20 => {
                let target = self.absolute(bus);
                let return_addr = self.pc.wrapping_sub(1);
                self.push_word(bus, return_addr);
                self.pc = target;
                6
            }
            0x60 => {
                self.pc = self.pop_word(bus).wrapping_add(1);
                6
            }
            0x40 => {
                self.status = (self.pop_byte(bus) | status::UNUSED) & !status::BREAK;
                self.pc = self.pop_word(bus);
                6
            }
            0xd0 => self.branch(bus, self.status & status::ZERO == 0),
            0xf0 => self.branch(bus, self.status & status::ZERO != 0),
            0x90 => self.branch(bus, self.status & status::CARRY == 0),
            0xb0 => self.branch(bus, self.status & status::CARRY != 0),
            0x10 => self.branch(bus, self.status & status::NEGATIVE == 0),
            0x30 => self.branch(bus, self.status & status::NEGATIVE != 0),
            0x50 => self.branch(bus, self.status & status::OVERFLOW == 0),
            0x70 => self.branch(bus, self.status & status::OVERFLOW != 0),
            other => {
                eprintln!(
                    "opcode table mismatch for {} (${other:02X}) at ${:04X}",
                    instruction.mnemonic,
                    self.pc - 1
                );
                self.stopped = true;
                2
            }
        };
        self.cycles += u64::from(cycles);
        cycles
    }

    pub fn nmi(&mut self, bus: &mut Bus) {
        self.push_word(bus, self.pc);
        self.push_byte(bus, self.status & !status::BREAK);
        self.status |= status::IRQ_DISABLE;
        let lo = u16::from(bus.cpu_read(0xfffa));
        let hi = u16::from(bus.cpu_read(0xfffb));
        self.pc = (hi << 8) | lo;
        self.cycles += 7;
    }

    pub fn irq(&mut self, bus: &mut Bus) {
        if self.status & status::IRQ_DISABLE == 0 {
            self.push_word(bus, self.pc);
            self.push_byte(bus, self.status & !status::BREAK);
            self.status |= status::IRQ_DISABLE;
            let lo = u16::from(bus.cpu_read(0xfffe));
            let hi = u16::from(bus.cpu_read(0xffff));
            self.pc = (hi << 8) | lo;
            self.cycles += 7;
        }
    }

    pub fn request_nmi(&mut self) {
        self.interrupt_request = InterruptRequest::Nmi;
    }

    pub fn request_irq(&mut self) {
        if self.interrupt_request != InterruptRequest::Nmi {
            self.interrupt_request = InterruptRequest::Irq;
        }
    }

    fn service_interrupt(&mut self, bus: &mut Bus) -> Option<u8> {
        match self.interrupt_request {
            InterruptRequest::None => None,
            InterruptRequest::Nmi => {
                self.interrupt_request = InterruptRequest::None;
                self.nmi(bus);
                Some(7)
            }
            InterruptRequest::Irq if self.status & status::IRQ_DISABLE == 0 => {
                self.interrupt_request = InterruptRequest::None;
                self.irq(bus);
                Some(7)
            }
            InterruptRequest::Irq => None,
        }
    }

    pub fn snapshot(&self) -> CpuState {
        CpuState {
            a: self.a,
            x: self.x,
            y: self.y,
            pc: self.pc,
            sp: self.sp,
            status: self.status,
            cycles: self.cycles,
            interrupt_request: self.interrupt_request,
            stopped: self.stopped,
        }
    }

    pub fn restore(&mut self, state: &CpuState) {
        self.a = state.a;
        self.x = state.x;
        self.y = state.y;
        self.pc = state.pc;
        self.sp = state.sp;
        self.status = state.status;
        self.cycles = state.cycles;
        self.interrupt_request = state.interrupt_request;
        self.stopped = state.stopped;
    }

    fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let value = bus.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = u16::from(self.fetch_byte(bus));
        let hi = u16::from(self.fetch_byte(bus));
        (hi << 8) | lo
    }

    fn zero_page(&mut self, bus: &mut Bus) -> u16 {
        u16::from(self.fetch_byte(bus))
    }

    fn zero_page_x(&mut self, bus: &mut Bus) -> u16 {
        u16::from(self.fetch_byte(bus).wrapping_add(self.x))
    }

    fn zero_page_y(&mut self, bus: &mut Bus) -> u16 {
        u16::from(self.fetch_byte(bus).wrapping_add(self.y))
    }

    fn absolute(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_word(bus)
    }

    fn absolute_x(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_word(bus).wrapping_add(u16::from(self.x))
    }

    fn absolute_y(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_word(bus).wrapping_add(u16::from(self.y))
    }

    fn indexed_indirect(&mut self, bus: &mut Bus) -> u16 {
        let ptr = self.fetch_byte(bus).wrapping_add(self.x);
        self.read_zero_page_word(bus, ptr)
    }

    fn indirect_indexed(&mut self, bus: &mut Bus) -> u16 {
        let ptr = self.fetch_byte(bus);
        self.read_zero_page_word(bus, ptr)
            .wrapping_add(u16::from(self.y))
    }

    fn read_zero_page_word(&mut self, bus: &mut Bus, ptr: u8) -> u16 {
        let lo = u16::from(bus.cpu_read(u16::from(ptr)));
        let hi = u16::from(bus.cpu_read(u16::from(ptr.wrapping_add(1))));
        (hi << 8) | lo
    }

    fn read_indirect_jmp(&mut self, bus: &mut Bus, ptr: u16) -> u16 {
        let lo = u16::from(bus.cpu_read(ptr));
        let hi_addr = (ptr & 0xff00) | (ptr.wrapping_add(1) & 0x00ff);
        let hi = u16::from(bus.cpu_read(hi_addr));
        (hi << 8) | lo
    }

    fn read_zero_page(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.zero_page(bus);
        bus.cpu_read(addr)
    }

    fn read_zero_page_x(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.zero_page_x(bus);
        bus.cpu_read(addr)
    }

    fn read_absolute(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.absolute(bus);
        bus.cpu_read(addr)
    }

    fn read_absolute_x(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.absolute_x(bus);
        bus.cpu_read(addr)
    }

    fn read_absolute_y(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.absolute_y(bus);
        bus.cpu_read(addr)
    }

    fn read_indexed_indirect(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.indexed_indirect(bus);
        bus.cpu_read(addr)
    }

    fn read_indirect_indexed(&mut self, bus: &mut Bus) -> u8 {
        let addr = self.indirect_indexed(bus);
        bus.cpu_read(addr)
    }

    fn branch(&mut self, bus: &mut Bus, take: bool) -> u8 {
        let offset = self.fetch_byte(bus) as i8;
        if take {
            self.pc = self.pc.wrapping_add_signed(i16::from(offset));
            3
        } else {
            2
        }
    }

    fn brk(&mut self, bus: &mut Bus) {
        self.pc = self.pc.wrapping_add(1);
        self.push_word(bus, self.pc);
        self.push_byte(bus, self.status | status::BREAK | status::UNUSED);
        self.status |= status::IRQ_DISABLE;
        let lo = u16::from(bus.cpu_read(0xfffe));
        let hi = u16::from(bus.cpu_read(0xffff));
        self.pc = (hi << 8) | lo;
    }

    fn push_byte(&mut self, bus: &mut Bus, value: u8) {
        bus.cpu_write(0x0100 | u16::from(self.sp), value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn push_word(&mut self, bus: &mut Bus, value: u16) {
        self.push_byte(bus, (value >> 8) as u8);
        self.push_byte(bus, value as u8);
    }

    fn pop_byte(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.cpu_read(0x0100 | u16::from(self.sp))
    }

    fn pop_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = u16::from(self.pop_byte(bus));
        let hi = u16::from(self.pop_byte(bus));
        (hi << 8) | lo
    }

    fn update_zero_negative(&mut self, value: u8) {
        if value == 0 {
            self.status |= status::ZERO;
        } else {
            self.status &= !status::ZERO;
        }
        if value & 0x80 != 0 {
            self.status |= status::NEGATIVE;
        } else {
            self.status &= !status::NEGATIVE;
        }
    }

    fn compare(&mut self, register: u8, value: u8) {
        let result = register.wrapping_sub(value);
        if register >= value {
            self.status |= status::CARRY;
        } else {
            self.status &= !status::CARRY;
        }
        self.update_zero_negative(result);
    }

    fn adc(&mut self, value: u8) {
        let carry = u8::from(self.status & status::CARRY != 0);
        let sum = u16::from(self.a) + u16::from(value) + u16::from(carry);
        let result = sum as u8;

        if sum > 0xff {
            self.status |= status::CARRY;
        } else {
            self.status &= !status::CARRY;
        }
        if ((self.a ^ result) & (value ^ result) & 0x80) != 0 {
            self.status |= status::OVERFLOW;
        } else {
            self.status &= !status::OVERFLOW;
        }

        self.a = result;
        self.update_zero_negative(self.a);
    }

    fn sbc(&mut self, value: u8) {
        self.adc(!value);
    }

    fn bit(&mut self, value: u8) {
        if self.a & value == 0 {
            self.status |= status::ZERO;
        } else {
            self.status &= !status::ZERO;
        }
        self.status = (self.status & !(status::NEGATIVE | status::OVERFLOW))
            | (value & (status::NEGATIVE | status::OVERFLOW));
    }

    fn inc_memory(&mut self, bus: &mut Bus, addr: u16) {
        let value = bus.cpu_read(addr).wrapping_add(1);
        bus.cpu_write(addr, value);
        self.update_zero_negative(value);
    }

    fn dec_memory(&mut self, bus: &mut Bus, addr: u16) {
        let value = bus.cpu_read(addr).wrapping_sub(1);
        bus.cpu_write(addr, value);
        self.update_zero_negative(value);
    }

    fn shift_memory(&mut self, bus: &mut Bus, addr: u16, op: fn(&mut Cpu, u8) -> u8) {
        let value = bus.cpu_read(addr);
        let shifted = op(self, value);
        bus.cpu_write(addr, shifted);
    }

    fn asl_value(&mut self, value: u8) -> u8 {
        if value & 0x80 != 0 {
            self.status |= status::CARRY;
        } else {
            self.status &= !status::CARRY;
        }
        let result = value << 1;
        self.update_zero_negative(result);
        result
    }

    fn lsr_value(&mut self, value: u8) -> u8 {
        if value & 0x01 != 0 {
            self.status |= status::CARRY;
        } else {
            self.status &= !status::CARRY;
        }
        let result = value >> 1;
        self.update_zero_negative(result);
        result
    }

    fn rol_value(&mut self, value: u8) -> u8 {
        let carry_in = u8::from(self.status & status::CARRY != 0);
        if value & 0x80 != 0 {
            self.status |= status::CARRY;
        } else {
            self.status &= !status::CARRY;
        }
        let result = (value << 1) | carry_in;
        self.update_zero_negative(result);
        result
    }

    fn ror_value(&mut self, value: u8) -> u8 {
        let carry_in = if self.status & status::CARRY != 0 {
            0x80
        } else {
            0
        };
        if value & 0x01 != 0 {
            self.status |= status::CARRY;
        } else {
            self.status &= !status::CARRY;
        }
        let result = (value >> 1) | carry_in;
        self.update_zero_negative(result);
        result
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuState {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub status: u8,
    pub cycles: u64,
    pub interrupt_request: InterruptRequest,
    pub stopped: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bus::Bus, mapper::mapper0::Mapper0, ppu::Ppu, rom::Mirroring};

    fn test_bus(program: &[u8]) -> Bus {
        let mut prg = vec![0xea; 0x8000];
        prg[..program.len()].copy_from_slice(program);
        prg[0x7ffc] = 0x00;
        prg[0x7ffd] = 0x80;
        bus_with_prg(prg)
    }

    fn bus_with_prg(prg: Vec<u8>) -> Bus {
        Bus::new(
            Ppu::new(Mirroring::Horizontal),
            crate::apu::Apu::default(),
            crate::input::Controller::default(),
            Box::new(Mapper0::new(prg, vec![0; 0x2000], true)),
        )
    }

    #[test]
    fn reset_reads_vector() {
        let mut bus = test_bus(&[0xea]);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        assert_eq!(cpu.pc, 0x8000);
    }

    #[test]
    fn lda_immediate_sets_accumulator_and_flags() {
        let mut bus = test_bus(&[0xa9, 0x80]);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x80);
        assert_ne!(cpu.status & status::NEGATIVE, 0);
    }

    #[test]
    fn indexed_indirect_and_indirect_indexed_addressing_work() {
        let mut bus = test_bus(&[
            0xa2, 0x04, 0xa0, 0x01, 0xa9, 0x34, 0x85, 0x24, 0xa9, 0x12, 0x85, 0x25, 0xa9, 0xab,
            0x8d, 0x35, 0x12, 0xa1, 0x20, 0xb1, 0x24,
        ]);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        for _ in 0..10 {
            cpu.step(&mut bus);
        }
        assert_eq!(cpu.a, 0xab);
    }

    #[test]
    fn adc_sbc_and_flags_work() {
        let mut bus = test_bus(&[0x18, 0xa9, 0x40, 0x69, 0x40, 0xe9, 0x01]);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        for _ in 0..5 {
            cpu.step(&mut bus);
        }
        assert_eq!(cpu.a, 0x7e);
        assert_eq!(cpu.status & status::CARRY, status::CARRY);
        assert_eq!(cpu.status & status::NEGATIVE, 0);
    }

    #[test]
    fn stack_jsr_rts_and_pha_pla_work() {
        let mut bus = test_bus(&[
            0x20, 0x06, 0x80, 0xa9, 0x7f, 0xea, 0xa9, 0x42, 0x48, 0xa9, 0x00, 0x68, 0x60,
        ]);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        for _ in 0..7 {
            cpu.step(&mut bus);
        }
        assert_eq!(cpu.a, 0x7f);
    }

    #[test]
    fn nmi_pushes_state_and_jumps_to_vector() {
        let mut prg = vec![0xea; 0x8000];
        prg[0x7ffa] = 0x34;
        prg[0x7ffb] = 0x12;
        prg[0x7ffc] = 0x00;
        prg[0x7ffd] = 0x80;
        let mut bus = bus_with_prg(prg);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);
        cpu.nmi(&mut bus);
        assert_eq!(cpu.pc, 0x1234);
        assert!(cpu.sp < 0xfd);
    }

    #[test]
    fn requested_nmi_is_serviced_before_next_instruction() {
        let mut prg = vec![0xea; 0x8000];
        prg[0x7ffa] = 0x78;
        prg[0x7ffb] = 0x56;
        prg[0x7ffc] = 0x00;
        prg[0x7ffd] = 0x80;
        let mut bus = bus_with_prg(prg);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);

        cpu.request_nmi();
        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 7);
        assert_eq!(cpu.pc, 0x5678);
        assert_eq!(cpu.snapshot().interrupt_request, InterruptRequest::None);
    }

    #[test]
    fn requested_irq_waits_until_interrupts_are_enabled() {
        let mut prg = vec![0xea; 0x8000];
        prg[0x7ffc] = 0x00;
        prg[0x7ffd] = 0x80;
        prg[0x7ffe] = 0xbc;
        prg[0x7fff] = 0x9a;
        let mut bus = bus_with_prg(prg);
        let mut cpu = Cpu::default();
        cpu.reset(&mut bus);

        cpu.request_irq();
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x8001);
        assert_eq!(cpu.snapshot().interrupt_request, InterruptRequest::Irq);

        cpu.status &= !status::IRQ_DISABLE;
        cpu.step(&mut bus);

        assert_eq!(cpu.pc, 0x9abc);
        assert_eq!(cpu.snapshot().interrupt_request, InterruptRequest::None);
    }
}
