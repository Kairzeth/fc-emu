#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Pulse1,
    Pulse2,
    Triangle,
    Noise,
    Dmc,
}

impl Channel {
    pub fn enable_mask(self) -> u8 {
        match self {
            Self::Pulse1 => 0x01,
            Self::Pulse2 => 0x02,
            Self::Triangle => 0x04,
            Self::Noise => 0x08,
            Self::Dmc => 0x10,
        }
    }
}
