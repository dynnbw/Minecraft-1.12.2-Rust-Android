//! Bedrock-style touch widget geometry in scaled coordinates.
//! Layout follows the Bedrock default touch layout (directions bottom-left,
//! jump bottom-right large, chat left-mid-upper, pause right-mid-upper,
//! backpack right of the 9th hotbar slot, close button inside the
//! inventory GUI). Sprite UVs follow docs/触控贴图.txt (touch.png 176x176).

use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// touch.png (176x176), shipped with the runtime assets.
pub fn touch_texture() -> ResourceLocation {
    ResourceLocation::new("minecraft", "textures/gui/touch.png")
}

pub const ATLAS: f32 = 176.0;

/// Sprite rect in texture pixels: (u, v, width, height).
pub type Sprite = (i32, i32, i32, i32);

// Row 1 (y=0, 22px): back, back-pressed, left, left-pressed, right, right-pressed, forward, forward-pressed
pub const BACK: Sprite = (0, 0, 22, 22);
pub const BACK_ACTIVE: Sprite = (22, 0, 22, 22);
pub const LEFT: Sprite = (44, 0, 22, 22);
pub const LEFT_ACTIVE: Sprite = (66, 0, 22, 22);
pub const RIGHT: Sprite = (88, 0, 22, 22);
pub const RIGHT_ACTIVE: Sprite = (110, 0, 22, 22);
pub const FORWARD: Sprite = (132, 0, 22, 22);
pub const FORWARD_ACTIVE: Sprite = (154, 0, 22, 22);
// Row 2 (y=22, 18px): left-back, right-back, left-forward, right-forward (+pressed)
pub const LEFT_BACK: Sprite = (0, 22, 18, 18);
pub const LEFT_BACK_ACTIVE: Sprite = (18, 22, 18, 18);
pub const RIGHT_BACK: Sprite = (36, 22, 18, 18);
pub const RIGHT_BACK_ACTIVE: Sprite = (54, 22, 18, 18);
pub const LEFT_FORWARD: Sprite = (72, 22, 18, 18);
pub const LEFT_FORWARD_ACTIVE: Sprite = (90, 22, 18, 18);
pub const RIGHT_FORWARD: Sprite = (108, 22, 18, 18);
pub const RIGHT_FORWARD_ACTIVE: Sprite = (126, 22, 18, 18);
// Row 3 (y=40, 18px): jump, flight, ascend, descend (+pressed)
pub const JUMP: Sprite = (0, 40, 18, 18);
pub const JUMP_ACTIVE: Sprite = (18, 40, 18, 18);
pub const FLIGHT: Sprite = (36, 40, 18, 18);
pub const FLIGHT_ACTIVE: Sprite = (54, 40, 18, 18);
pub const ASCEND: Sprite = (72, 40, 18, 18);
pub const ASCEND_ACTIVE: Sprite = (90, 40, 18, 18);
pub const DESCEND: Sprite = (108, 40, 18, 18);
pub const DESCEND_ACTIVE: Sprite = (126, 40, 18, 18);
// Row 4 (y=58, 18px): sneak, cross, esc-pause, chat (+pressed)
pub const SNEAK: Sprite = (0, 58, 18, 18);
pub const SNEAK_ACTIVE: Sprite = (18, 58, 18, 18);
pub const CROSS: Sprite = (36, 58, 18, 18);
pub const CROSS_ACTIVE: Sprite = (54, 58, 18, 18);
pub const ESC: Sprite = (72, 58, 18, 18);
pub const ESC_ACTIVE: Sprite = (90, 58, 18, 18);
pub const CHAT: Sprite = (108, 58, 18, 18);
pub const CHAT_ACTIVE: Sprite = (126, 58, 18, 18);
// Row 5 (y=76, 18px): touch settings (+pressed) — drawn on GuiOptions, not the world HUD
pub const SETTINGS: Sprite = (0, 76, 18, 18);
pub const SETTINGS_ACTIVE: Sprite = (18, 76, 18, 18);
// Bottom-left (y=154, 22px): backpack (+pressed)
pub const BACKPACK: Sprite = (0, 154, 22, 22);
pub const BACKPACK_ACTIVE: Sprite = (22, 154, 22, 22);

pub const fn pick(active: bool, idle: Sprite, act: Sprite) -> Sprite {
    if active { act } else { idle }
}

/// One of the 9 DPad grid cells (the center cell is unused).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpadDirection {
    Forward,
    Backward,
    Left,
    Right,
    LeftForward,
    RightForward,
    LeftBackward,
    RightBackward,
}

/// DPad cross grid: 3x3 grid of 44px cells with 8px channels, top row =
/// forward. `origin` is the grid top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DPadLayout {
    pub origin: (i32, i32),
    pub cell: i32,
    pub gap: i32,
}

impl DPadLayout {
    pub fn size(&self) -> i32 {
        self.cell * 3 + self.gap * 2
    }

    /// Which cell contains `position`: row0 = forward row, row2 = backward.
    pub fn direction_at(&self, position: (f64, f64)) -> Option<DpadDirection> {
        let p = (position.0 - self.origin.0 as f64, position.1 - self.origin.1 as f64);
        let b = self.cell as f64;
        let g = self.gap as f64;
        let in_h = |x0: f64, w: f64| p.0 >= x0 && p.0 < x0 + w;
        let in_v = |y0: f64, h: f64| p.1 >= y0 && p.1 < y0 + h;
        let col0 = (0.0, b + g);
        let col1 = (b + g, b);
        let col2 = (2.0 * b + g, b + g);
        let row0 = (0.0, b + g);
        let row1 = (b + g, b);
        let row2 = (2.0 * b + g, b + g);
        let hit = |c: (f64, f64), r: (f64, f64)| in_h(c.0, c.1) && in_v(r.0, r.1);
        if hit(col1, row0) {
            Some(DpadDirection::Forward)
        } else if hit(col1, row2) {
            Some(DpadDirection::Backward)
        } else if hit(col0, row1) {
            Some(DpadDirection::Left)
        } else if hit(col2, row1) {
            Some(DpadDirection::Right)
        } else if hit(col0, row0) {
            Some(DpadDirection::LeftForward)
        } else if hit(col2, row0) {
            Some(DpadDirection::RightForward)
        } else if hit(col0, row2) {
            Some(DpadDirection::LeftBackward)
        } else if hit(col2, row2) {
            Some(DpadDirection::RightBackward)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchWidget {
    Jump { rect: (i32, i32, i32, i32) },
    Sneak { rect: (i32, i32, i32, i32) },
    Chat { rect: (i32, i32, i32, i32) },
    Pause { rect: (i32, i32, i32, i32) },
    Backpack { rect: (i32, i32, i32, i32) },
    Ascend { rect: (i32, i32, i32, i32) },
    Descend { rect: (i32, i32, i32, i32) },
    /// Backpack-close button inside the inventory GUI.
    BackpackClose { rect: (i32, i32, i32, i32) },
}

pub fn hit_test(widget: &TouchWidget, position: (f64, f64)) -> bool {
    match widget {
        TouchWidget::Jump { rect } | TouchWidget::Sneak { rect }
        | TouchWidget::Chat { rect } | TouchWidget::Pause { rect }
        | TouchWidget::Backpack { rect } | TouchWidget::Ascend { rect }
        | TouchWidget::Descend { rect } | TouchWidget::BackpackClose { rect } => {
            let (x, y, w, h) = *rect;
            position.0 >= x as f64 && position.0 < (x + w) as f64
                && position.1 >= y as f64 && position.1 < (y + h) as f64
        }
    }
}

/// Bedrock-default geometry (scaled coordinates). Reference 600x270.
pub struct BedrockLayout {
    pub dpad: DPadLayout,
    pub jump: TouchWidget,
    pub sneak: TouchWidget,
    pub chat: TouchWidget,
    pub pause: TouchWidget,
    pub backpack: TouchWidget,
    pub ascend: TouchWidget,
    pub descend: TouchWidget,
    pub backpackClose: TouchWidget,
}

pub fn bedrock_geometry(width: i32, height: i32) -> BedrockLayout {
    // DPad: 3x3 grid, 44px cells, 8px gaps, LEFT_BOTTOM (12, 16).
    let cell = 44;
    let gap = 8;
    let dpad = DPadLayout {
        origin: (12, height - 16 - (cell * 3 + gap * 2)),
        cell,
        gap,
    };
    // Jump: large button bottom-right, RIGHT_BOTTOM (42, 68).
    let jumpRect = (width - 42 - 56, height - 68 - 56, 56, 56);
    // Sneak: left of jump, bottom-aligned.
    let sneakRect = (jumpRect.0 - 8 - 40, jumpRect.1 + 56 - 40, 40, 40);
    // Ascend/descend stack above the jump (shown while flying).
    let ascend = (jumpRect.0 + 8, jumpRect.1 - 8 - 40, 40, 40);
    let descend = (jumpRect.0 + 8, ascend.1 - 8 - 40, 40, 40);
    // Chat left-mid-upper, pause right-mid-upper (mirrored). Kept above the
    // DPad top edge (8px gap) so the touch HUD never overlaps the movement
    // grid; on tall screens it stays in the mid-upper band.
    let topY = (height / 2 - 60).min(height - 16 - (cell * 3 + gap * 2) - 8 - 40).max(12);
    let chat = (12, topY, 40, 40);
    let pause = (width - 12 - 40, topY, 40, 40);
    // Backpack: right of the 9th hotbar slot. Hotbar: 9 slots x 20px,
    // centered at the bottom, slot 9 spans [width/2+70, width/2+90].
    let backpack = (width / 2 + 90 + 8, height - 16 - 40, 40, 40);
    // Backpack-close: top-right inside the inventory GUI (176 wide,
    // centered), 20x20.
    let backpackClose = (width / 2 + 88 - 24, 44, 20, 20);
    BedrockLayout {
        dpad,
        jump: TouchWidget::Jump { rect: jumpRect },
        sneak: TouchWidget::Sneak { rect: sneakRect },
        chat: TouchWidget::Chat { rect: chat },
        pause: TouchWidget::Pause { rect: pause },
        backpack: TouchWidget::Backpack { rect: backpack },
        ascend: TouchWidget::Ascend { rect: ascend },
        descend: TouchWidget::Descend { rect: descend },
        backpackClose: TouchWidget::BackpackClose { rect: backpackClose },
    }
}

#[cfg(test)]
mod tests {
    use super::{bedrock_geometry, DpadDirection, TouchWidget, hit_test};

    /// TouchWidget is an enum, so extract the rect via match.
    fn rect(widget: TouchWidget) -> (i32, i32, i32, i32) {
        match widget {
            TouchWidget::Jump { rect } | TouchWidget::Sneak { rect }
            | TouchWidget::Chat { rect } | TouchWidget::Pause { rect }
            | TouchWidget::Backpack { rect } | TouchWidget::Ascend { rect }
            | TouchWidget::Descend { rect } | TouchWidget::BackpackClose { rect } => rect,
        }
    }

    #[test]
    fn bedrock_geometry_places_widgets() {
        let layout = bedrock_geometry(600, 270);
        // jump bottom-right (big button)
        let jump = rect(layout.jump);
        assert!(jump.0 > 450 && jump.1 > 130);
        // chat left, pause right, same vertical band
        let chat = rect(layout.chat);
        let pause = rect(layout.pause);
        assert_eq!(chat.0, 12);
        assert!(pause.0 > 500);
        assert_eq!(chat.1, pause.1);
        // backpack right of the 9th hotbar slot (hotbar x ends at 300)
        assert!(rect(layout.backpack).0 >= 300);
        // dpad bottom-left grid
        assert!(layout.dpad.origin.0 < 30 && layout.dpad.origin.1 > 90);
    }

    #[test]
    fn dpad_direction_at_maps_grid_cells() {
        let layout = bedrock_geometry(600, 270);
        let (ox, oy) = layout.dpad.origin;
        let f = |x: i32, y: i32| (x as f64, y as f64);
        // top-middle cell = forward
        assert_eq!(layout.dpad.direction_at(f(ox + 44 + 8 + 20, oy + 20)), Some(DpadDirection::Forward));
        // left-middle = left
        assert_eq!(layout.dpad.direction_at(f(ox + 20, oy + 44 + 8 + 20)), Some(DpadDirection::Left));
        // top-left corner = left-forward
        assert_eq!(layout.dpad.direction_at(f(ox + 20, oy + 20)), Some(DpadDirection::LeftForward));
        // outside
        assert_eq!(layout.dpad.direction_at(f(ox - 10, oy - 10)), None);
    }

    #[test]
    fn hit_test_checks_rect() {
        let control = TouchWidget::Jump { rect: (502, 146, 56, 56) };
        assert!(hit_test(&control, (510.0, 160.0)));
        assert!(!hit_test(&control, (490.0, 160.0)));
    }
}
