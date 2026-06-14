use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Button {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
}

impl Button {
    pub const ALL: [Button; 8] = [
        Button::A,
        Button::B,
        Button::Select,
        Button::Start,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
    ];

    fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveSlot {
    Slot(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppControlAction {
    Pause,
    Resume,
    TogglePause,
    SaveState,
    LoadState,
    SelectSaveSlot(SaveSlot),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    X,
    Z,
    Enter,
    RightShift,
    S,
    Tab,
    Space,
    P,
    F,
    L,
    F5,
    F9,
    Digit(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyMapping {
    Controller(Button),
    App(AppControlAction),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Controller {
    buttons: u8,
    strobe: bool,
    read_index: u8,
}

impl Controller {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_pressed(&self, button: Button) -> bool {
        self.buttons & button.mask() != 0
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        if pressed {
            self.buttons |= button.mask();
        } else {
            self.buttons &= !button.mask();
        }
    }

    pub fn apply_key(&mut self, key: KeyboardKey, pressed: bool) -> Option<AppControlAction> {
        match default_key_mapping(key) {
            Some(KeyMapping::Controller(button)) => {
                self.set_button(button, pressed);
                None
            }
            Some(KeyMapping::App(action)) => pressed.then_some(action),
            None => None,
        }
    }

    pub fn write_strobe(&mut self, value: u8) {
        self.strobe = value & 0x01 != 0;
        if self.strobe {
            self.read_index = 0;
        }
    }

    pub fn read_4016(&mut self) -> u8 {
        let bit = if self.read_index < Button::ALL.len() as u8 {
            u8::from(self.is_pressed(Button::ALL[self.read_index as usize]))
        } else {
            1
        };

        if self.strobe {
            self.read_index = 0;
        } else if self.read_index < Button::ALL.len() as u8 {
            self.read_index += 1;
        }

        bit
    }

    pub fn strobe(&self) -> bool {
        self.strobe
    }

    pub fn read_index(&self) -> u8 {
        self.read_index
    }
}

pub fn default_key_mapping(key: KeyboardKey) -> Option<KeyMapping> {
    let mapping = match key {
        KeyboardKey::ArrowUp => KeyMapping::Controller(Button::Up),
        KeyboardKey::ArrowDown => KeyMapping::Controller(Button::Down),
        KeyboardKey::ArrowLeft => KeyMapping::Controller(Button::Left),
        KeyboardKey::ArrowRight => KeyMapping::Controller(Button::Right),
        KeyboardKey::X => KeyMapping::Controller(Button::A),
        KeyboardKey::Z => KeyMapping::Controller(Button::B),
        KeyboardKey::Enter => KeyMapping::Controller(Button::Start),
        KeyboardKey::RightShift | KeyboardKey::S | KeyboardKey::Tab => {
            KeyMapping::Controller(Button::Select)
        }
        KeyboardKey::Space | KeyboardKey::P => KeyMapping::App(AppControlAction::TogglePause),
        KeyboardKey::F => KeyMapping::App(AppControlAction::SaveState),
        KeyboardKey::L => KeyMapping::App(AppControlAction::LoadState),
        KeyboardKey::F5 => KeyMapping::App(AppControlAction::SaveState),
        KeyboardKey::F9 => KeyMapping::App(AppControlAction::LoadState),
        KeyboardKey::Digit(slot @ 0..=9) => {
            KeyMapping::App(AppControlAction::SelectSaveSlot(SaveSlot::Slot(slot)))
        }
        KeyboardKey::Digit(_) => return None,
    };

    Some(mapping)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_button_state() {
        let mut controller = Controller::new();

        controller.set_button(Button::A, true);
        controller.set_button(Button::Right, true);
        assert!(controller.is_pressed(Button::A));
        assert!(controller.is_pressed(Button::Right));
        assert!(!controller.is_pressed(Button::B));

        controller.set_button(Button::A, false);
        assert!(!controller.is_pressed(Button::A));
        assert!(controller.is_pressed(Button::Right));
    }

    #[test]
    fn maps_default_keyboard_buttons_and_app_controls() {
        let mut controller = Controller::new();

        assert_eq!(controller.apply_key(KeyboardKey::X, true), None);
        assert!(controller.is_pressed(Button::A));

        assert_eq!(
            controller.apply_key(KeyboardKey::Space, true),
            Some(AppControlAction::TogglePause)
        );
        assert_eq!(
            controller.apply_key(KeyboardKey::F, true),
            Some(AppControlAction::SaveState)
        );
        assert_eq!(
            controller.apply_key(KeyboardKey::L, true),
            Some(AppControlAction::LoadState)
        );
        assert_eq!(
            controller.apply_key(KeyboardKey::F5, true),
            Some(AppControlAction::SaveState)
        );
        assert_eq!(
            controller.apply_key(KeyboardKey::F9, true),
            Some(AppControlAction::LoadState)
        );
        assert_eq!(
            controller.apply_key(KeyboardKey::Digit(3), true),
            Some(AppControlAction::SelectSaveSlot(SaveSlot::Slot(3)))
        );

        assert!(controller.is_pressed(Button::A));
        assert!(!controller.is_pressed(Button::Start));

        assert_eq!(controller.apply_key(KeyboardKey::S, true), None);
        assert!(controller.is_pressed(Button::Select));
        controller.set_button(Button::Select, false);
        assert_eq!(controller.apply_key(KeyboardKey::Tab, true), None);
        assert!(controller.is_pressed(Button::Select));
    }

    #[test]
    fn strobe_keeps_reading_a_button() {
        let mut controller = Controller::new();
        controller.set_button(Button::A, true);
        controller.set_button(Button::B, false);

        controller.write_strobe(1);

        assert!(controller.strobe());
        assert_eq!(controller.read_4016(), 1);
        assert_eq!(controller.read_4016(), 1);
        assert_eq!(controller.read_index(), 0);
    }

    #[test]
    fn reads_buttons_in_nes_serial_order() {
        let mut controller = Controller::new();
        controller.set_button(Button::A, true);
        controller.set_button(Button::Select, true);
        controller.set_button(Button::Up, true);
        controller.set_button(Button::Right, true);

        controller.write_strobe(1);
        controller.write_strobe(0);

        let bits: Vec<u8> = (0..8).map(|_| controller.read_4016()).collect();
        assert_eq!(bits, vec![1, 0, 1, 0, 1, 0, 0, 1]);
    }

    #[test]
    fn reads_one_after_eight_buttons() {
        let mut controller = Controller::new();
        controller.write_strobe(0);

        for _ in 0..8 {
            controller.read_4016();
        }

        assert_eq!(controller.read_4016(), 1);
        assert_eq!(controller.read_4016(), 1);
    }

    #[test]
    fn app_controls_do_not_mutate_controller_buttons_on_release_or_press() {
        let mut controller = Controller::new();

        assert_eq!(
            controller.apply_key(KeyboardKey::P, true),
            Some(AppControlAction::TogglePause)
        );
        assert_eq!(controller.apply_key(KeyboardKey::P, false), None);

        for button in Button::ALL {
            assert!(!controller.is_pressed(button));
        }
    }
}
