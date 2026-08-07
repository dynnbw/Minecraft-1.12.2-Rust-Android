//! Multi-touch pointer tracking.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pointer {
    position: (f64, f64),
}

#[derive(Debug, Clone, Default)]
pub struct PointerState {
    pointers: HashMap<u64, Pointer>,
}

impl PointerState {
    pub fn new() -> Self { Self::default() }

    pub fn started(&mut self, id: u64, position: (f64, f64)) {
        self.pointers.insert(id, Pointer { position });
    }

    pub fn moved(&mut self, id: u64, position: (f64, f64)) {
        if let Some(pointer) = self.pointers.get_mut(&id) {
            pointer.position = position;
        }
    }

    pub fn ended(&mut self, id: u64) {
        self.pointers.remove(&id);
    }

    pub fn is_down(&self, id: u64) -> bool {
        self.pointers.contains_key(&id)
    }

    pub fn position(&self, id: u64) -> Option<(f64, f64)> {
        self.pointers.get(&id).map(|pointer| pointer.position)
    }

    pub fn reset(&mut self) {
        self.pointers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::PointerState;

    #[test]
    fn started_moved_ended_tracks_lifecycle() {
        let mut state = PointerState::new();
        state.started(0, (100.0, 200.0));
        assert!(state.is_down(0));
        state.moved(0, (120.0, 210.0));
        assert_eq!(state.position(0), Some((120.0, 210.0)));
        state.ended(0);
        assert!(!state.is_down(0));
    }
}
