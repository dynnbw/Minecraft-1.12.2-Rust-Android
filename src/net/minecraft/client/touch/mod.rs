//! Bedrock-style touch layer (Android). Controls map directly onto the
//! vanilla 1.12.2 keyboard bindings (MovementKeyState / KeyBinding
//! semantics); layout follows the Bedrock default touch layout.

#[path = "TouchConfig.rs"] pub mod TouchConfig;
#[path = "Widgets.rs"] pub mod Widgets;
#[path = "PointerState.rs"] pub mod PointerState;
#[path = "KeyState.rs"] pub mod KeyState;
#[path = "Draw.rs"] pub mod Draw;

#[cfg(any(target_os = "android", test))]
use std::collections::HashMap;

/// Runtime state of the active touch layer. Created lazily on the first
/// enabled touch and dropped when the app suspends. Also compiled for
/// `cargo test` on desktop targets so the widget-routing logic is covered
/// by unit tests on the dev machine.
#[cfg(any(target_os = "android", test))]
pub struct TouchRuntime {
    pub keys: KeyState::KeyState,
    pointers: PointerState::PointerState,
    /// Per-pointer widget ownership (drag-in/drag-out switching).
    buttonHeld: HashMap<u64, HeldKind>,
    /// Sneak long-press lock start tick.
    sneakLockStart: Option<i32>,
    lastSize: (i32, i32),
    layout: Widgets::BedrockLayout,
}

#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeldKind {
    Dpad(Widgets::DpadDirection),
    Jump,
    Sneak,
    Ascend,
    Descend,
}

#[cfg(any(target_os = "android", test))]
impl Default for TouchRuntime {
    fn default() -> Self {
        Self {
            keys: KeyState::KeyState::default(),
            pointers: PointerState::PointerState::new(),
            buttonHeld: HashMap::new(),
            sneakLockStart: None,
            lastSize: (0, 0),
            layout: Widgets::bedrock_geometry(600, 270),
        }
    }
}

#[cfg(any(target_os = "android", test))]
impl TouchRuntime {
    pub fn new() -> Self { Self::default() }

    fn rebuildLayout(&mut self, size: (i32, i32)) {
        if size != self.lastSize {
            self.layout = Widgets::bedrock_geometry(size.0, size.1);
            self.lastSize = size;
        }
    }

    fn hit(&self, position: (f64, f64), flying: bool) -> Option<HeldKind> {
        // The DPad center cell is the sneak button (below forward, left of
        // right); the outer eight cells are directions.
        if Widgets::hit_test(&self.layout.sneak, position) { return Some(HeldKind::Sneak); }
        if let Some(direction) = self.layout.dpad.direction_at(position) {
            return Some(HeldKind::Dpad(direction));
        }
        if Widgets::hit_test(&self.layout.jump, position) { return Some(HeldKind::Jump); }
        // Ascend/descend exist only while flying; outside flight they are
        // neither drawn nor hittable (the touch falls through).
        if flying && Widgets::hit_test(&self.layout.ascend, position) { return Some(HeldKind::Ascend); }
        if flying && Widgets::hit_test(&self.layout.descend, position) { return Some(HeldKind::Descend); }
        None
    }

    /// Touch Started/Moved on a widget: returns true when consumed by a
    /// widget (false = fall through to the legacy bridge). `flying` gates
    /// the ascend/descend buttons (only hittable while flying).
    pub fn handle_touch_widget(
        &mut self,
        phase: winit::event::TouchPhase,
        id: u64,
        position: (f64, f64),
        size: (i32, i32),
        tick: i32,
        flying: bool,
    ) -> bool {
        self.rebuildLayout(size);
        match phase {
            winit::event::TouchPhase::Started => {
                if let Some(kind) = self.hit(position, flying) {
                    self.buttonHeld.insert(id, kind);
                    self.pointers.started(id, position);
                    self.apply_one(kind);
                    if kind == HeldKind::Sneak {
                        self.sneakLockStart = Some(tick);
                    }
                    return true;
                }
                false
            }
            winit::event::TouchPhase::Moved => {
                if let Some(&held) = self.buttonHeld.get(&id) {
                    self.pointers.moved(id, position);
                    let hit = self.hit(position, flying);
                    if hit != Some(held) {
                        if let Some(next) = hit {
                            self.buttonHeld.insert(id, next);
                        } else {
                            self.buttonHeld.remove(&id);
                            self.pointers.ended(id);
                        }
                        self.reapply_all();
                    }
                    return true;
                }
                false
            }
            winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                if self.buttonHeld.remove(&id).is_some() {
                    self.pointers.ended(id);
                    if self.buttonHeld.is_empty() {
                        self.keys.set_dpad(None);
                        self.keys.jump = false;
                        self.keys.sneak = false;
                        self.sneakLockStart = None;
                    } else {
                        self.reapply_all();
                    }
                    return true;
                }
                false
            }
        }
    }

    fn apply_one(&mut self, kind: HeldKind) {
        match kind {
            HeldKind::Dpad(direction) => self.keys.set_dpad(Some(direction)),
            HeldKind::Jump | HeldKind::Ascend => {
                self.keys.jump = true;
                self.keys.set_dpad(None);
            }
            HeldKind::Sneak | HeldKind::Descend => {
                self.keys.sneak = true;
                self.keys.set_dpad(None);
            }
        }
    }

    /// Re-applies the still-held widgets after a drag switch or release.
    fn reapply_all(&mut self) {
        self.keys.set_dpad(None);
        self.keys.jump = false;
        self.keys.sneak = false;
        for kind in self.buttonHeld.values() {
            match kind {
                HeldKind::Dpad(direction) => self.keys.set_dpad(Some(*direction)),
                HeldKind::Jump | HeldKind::Ascend => self.keys.jump = true,
                HeldKind::Sneak | HeldKind::Descend => self.keys.sneak = true,
            }
        }
    }

    /// One client tick: consume one-shot actions, keep held keys armed,
    /// advance the sneak long-press lock (>= 10 ticks).
    pub fn tick(&mut self, tick: i32) {
        self.keys.tick();
        if let Some(start) = self.sneakLockStart {
            if tick - start >= 10 {
                self.keys.sneakLocked = true;
            }
        }
        let held = self.buttonHeld.values().copied().collect::<Vec<_>>();
        for kind in held {
            if matches!(kind, HeldKind::Jump | HeldKind::Ascend) {
                self.keys.jump = true;
            }
            if matches!(kind, HeldKind::Sneak | HeldKind::Descend) {
                self.keys.sneak = true;
            }
        }
    }

    pub fn layout(&self) -> &Widgets::BedrockLayout {
        &self.layout
    }

    /// The currently held DPad direction, if any. Mirrors the drawn HUD
    /// highlight to the widget hit-testing so both stay consistent.
    pub fn held_direction(&self) -> Option<Widgets::DpadDirection> {
        self.buttonHeld.values().find_map(|kind| match kind {
            HeldKind::Dpad(direction) => Some(*direction),
            _ => None,
        })
    }

    pub fn reset(&mut self) {
        self.pointers.reset();
        self.keys = KeyState::KeyState::default();
        self.buttonHeld.clear();
        self.sneakLockStart = None;
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::TouchRuntime;
    use crate::net::minecraft::client::touch::Widgets::{bedrock_geometry, TouchWidget};

    fn jump_center() -> (f64, f64) {
        let layout = bedrock_geometry(600, 270);
        let rect = match layout.jump { TouchWidget::Jump { rect } => rect, _ => unreachable!() };
        ((rect.0 + rect.2 / 2) as f64, (rect.1 + rect.3 / 2) as f64)
    }

    #[test]
    fn started_on_jump_sets_jump_until_released() {
        let mut runtime = TouchRuntime::new();
        let layout = bedrock_geometry(600, 270);
        let center = jump_center();
        assert!(runtime.handle_touch_widget(winit::event::TouchPhase::Started, 0, center, (600, 270), 0, false));
        assert!(runtime.keys.jump);
        runtime.handle_touch_widget(winit::event::TouchPhase::Ended, 0, center, (600, 270), 1, false);
        assert!(!runtime.keys.jump);
    }

    #[test]
    fn started_outside_widgets_falls_through() {
        let mut runtime = TouchRuntime::new();
        // Top-left corner is empty (no widget there).
        assert!(!runtime.handle_touch_widget(winit::event::TouchPhase::Started, 0, (5.0, 5.0), (600, 270), 0, false));
    }
}
