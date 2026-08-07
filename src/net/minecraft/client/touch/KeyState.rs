//! Synthesized key states: the touch controls map directly onto the
//! vanilla key bindings (MovementKeyState fields + action flags).

use super::Widgets::DpadDirection;

#[derive(Debug, Clone, Default)]
pub struct KeyState {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    /// One-shot actions, consumed each client tick.
    pub chat: bool,
    pub pause: bool,
    pub backpack: bool,
    pub closeContainer: bool,
    /// Long-press sneak lock.
    pub sneakLocked: bool,
}

impl KeyState {
    /// Applies the DPad cell to the movement flags.
    pub fn set_dpad(&mut self, direction: Option<DpadDirection>) {
        self.forward = false;
        self.back = false;
        self.left = false;
        self.right = false;
        match direction {
            Some(DpadDirection::Forward) | Some(DpadDirection::LeftForward) | Some(DpadDirection::RightForward) => self.forward = true,
            _ => {}
        }
        match direction {
            Some(DpadDirection::Backward) | Some(DpadDirection::LeftBackward) | Some(DpadDirection::RightBackward) => self.back = true,
            _ => {}
        }
        match direction {
            Some(DpadDirection::Left) | Some(DpadDirection::LeftForward) | Some(DpadDirection::LeftBackward) => self.left = true,
            _ => {}
        }
        match direction {
            Some(DpadDirection::Right) | Some(DpadDirection::RightForward) | Some(DpadDirection::RightBackward) => self.right = true,
            _ => {}
        }
    }

    /// Consumes the one-shot action flags (click lasts one client tick).
    pub fn tick(&mut self) {
        self.chat = false;
        self.pause = false;
        self.backpack = false;
        self.closeContainer = false;
    }
}

#[cfg(test)]
mod tests {
    use super::KeyState;
    use crate::net::minecraft::client::touch::Widgets::DpadDirection;

    #[test]
    fn dpad_maps_to_movement_flags() {
        let mut state = KeyState::default();
        state.set_dpad(Some(DpadDirection::LeftForward));
        assert!(state.forward && state.left && !state.back && !state.right);
        state.set_dpad(Some(DpadDirection::Backward));
        assert!(state.back && !state.forward);
        state.set_dpad(None);
        assert!(!state.forward && !state.back && !state.left && !state.right);
    }

    #[test]
    fn actions_are_consumed_each_tick() {
        let mut state = KeyState::default();
        state.chat = true;
        state.tick();
        assert!(!state.chat);
    }
}
