//! Touch HUD rendering onto the GuiDrawList using touch.png sprites
//! (UVs follow docs/触控贴图.txt).

use crate::vulkan::GuiDrawList::GuiDrawList;
use super::Widgets::{
    ATLAS, ASCEND, ASCEND_ACTIVE, BACK, BACK_ACTIVE, BACKPACK, BACKPACK_ACTIVE,
    bedrock_geometry, CHAT, CHAT_ACTIVE, CROSS, CROSS_ACTIVE, DESCEND, DESCEND_ACTIVE,
    DPadLayout, DpadDirection, ESC, ESC_ACTIVE, FLIGHT, FLIGHT_ACTIVE, FORWARD, FORWARD_ACTIVE,
    JUMP, JUMP_ACTIVE, LEFT, LEFT_ACTIVE, LEFT_BACK, LEFT_BACK_ACTIVE,
    LEFT_FORWARD, LEFT_FORWARD_ACTIVE, pick, RIGHT, RIGHT_ACTIVE, RIGHT_BACK, RIGHT_BACK_ACTIVE,
    RIGHT_FORWARD, RIGHT_FORWARD_ACTIVE, SNEAK, SNEAK_ACTIVE, Sprite, TouchWidget,
    touch_texture,
};

/// Returns (dpad rect, button rects) for overlap checks.
pub fn button_rects(width: i32, height: i32) -> ((i32, i32, i32, i32), Vec<(i32, i32, i32, i32)>) {
    let layout = bedrock_geometry(width, height);
    let size = layout.dpad.size();
    let dpad = (layout.dpad.origin.0, layout.dpad.origin.1, layout.dpad.origin.0 + size, layout.dpad.origin.1 + size);
    // Sneak lives in the DPad's center cell, so it is part of the pad and
    // not listed among the outer buttons.
    let buttons = [layout.jump, layout.chat, layout.pause, layout.backpack, layout.ascend, layout.descend]
        .iter().map(|widget| rect_of(widget)).collect();
    (dpad, buttons)
}

pub fn rect_of(widget: &TouchWidget) -> (i32, i32, i32, i32) {
    match widget {
        TouchWidget::Jump { rect } | TouchWidget::Sneak { rect } | TouchWidget::Chat { rect }
        | TouchWidget::Pause { rect } | TouchWidget::Backpack { rect }
        | TouchWidget::Ascend { rect } | TouchWidget::Descend { rect }
        | TouchWidget::BackpackClose { rect } | TouchWidget::WinGameSkip { rect } => *rect,
    }
}

/// Stretches a sprite onto the target rect. The source region is the
/// sprite's own (sw x sh) pixels, stretched to the (w x h) target — unlike
/// `draw_modal_rect_with_custom_sized_texture`, which samples an area equal
/// to the draw size and would cut neighboring sprites out of the atlas.
pub fn draw_sprite(drawList: &mut GuiDrawList, x: i32, y: i32, w: i32, h: i32, (u, v, sw, sh): Sprite) {
    let inv_w = 1.0 / ATLAS;
    let inv_h = 1.0 / ATLAS;
    drawList.push_textured_quad(
        touch_texture(),
        [
            (x as f32, (y + h) as f32, u as f32 * inv_w, (v + sh) as f32 * inv_h, 0xFFFF_FFFF),
            ((x + w) as f32, (y + h) as f32, (u + sw) as f32 * inv_w, (v + sh) as f32 * inv_h, 0xFFFF_FFFF),
            ((x + w) as f32, y as f32, (u + sw) as f32 * inv_w, v as f32 * inv_h, 0xFFFF_FFFF),
            (x as f32, y as f32, u as f32 * inv_w, v as f32 * inv_h, 0xFFFF_FFFF),
        ],
    );
}

/// Draws the DPad cross grid; the center cell is skipped. `direction` is
/// the currently held cell (highlighted with its active sprite).
pub fn draw_dpad(drawList: &mut GuiDrawList, dpad: DPadLayout, direction: Option<DpadDirection>) {
    let cell = dpad.cell;
    let gap = dpad.gap;
    let (ox, oy) = dpad.origin;
    // Column/row offsets: diagonal cells are 18/22 of the main cells; the
    // second diagonal column/row starts after main + gap so both gaps are
    // symmetric.
    let offsets = [0, dpad.diagonal + gap, dpad.diagonal + gap + cell + gap];
    let mut index = 0;
    for row in 0..3 {
        for col in 0..3 {
            // The center cell is the sneak button (drawn separately); it must
            // not advance the sprite index, or every later cell shifts by one.
            if (row, col) == (1, 1) { continue; }
            let x = ox + offsets[col];
            let y = oy + offsets[row];
            let size = if col == 1 || row == 1 { cell } else { dpad.diagonal };
            let active = direction == cell_direction(index);
            let sprite = cell_sprite(index, active);
            draw_sprite(drawList, x, y, size, size, sprite);
            index += 1;
        }
    }
}

fn cell_direction(index: i32) -> Option<DpadDirection> {
    // Row-major, center skipped: 0=left-forward, 1=forward, 2=right-forward,
    // 3=left, 4=right, 5=left-backward, 6=backward, 7=right-backward.
    Some(match index {
        0 => DpadDirection::LeftForward,
        1 => DpadDirection::Forward,
        2 => DpadDirection::RightForward,
        3 => DpadDirection::Left,
        4 => DpadDirection::Right,
        5 => DpadDirection::LeftBackward,
        6 => DpadDirection::Backward,
        7 => DpadDirection::RightBackward,
        _ => return None,
    })
}

fn cell_sprite(index: i32, active: bool) -> Sprite {
    match index {
        0 => pick(active, LEFT_FORWARD, LEFT_FORWARD_ACTIVE),
        1 => pick(active, FORWARD, FORWARD_ACTIVE),
        2 => pick(active, RIGHT_FORWARD, RIGHT_FORWARD_ACTIVE),
        3 => pick(active, LEFT, LEFT_ACTIVE),
        4 => pick(active, RIGHT, RIGHT_ACTIVE),
        5 => pick(active, LEFT_BACK, LEFT_BACK_ACTIVE),
        6 => pick(active, BACK, BACK_ACTIVE),
        7 => pick(active, RIGHT_BACK, RIGHT_BACK_ACTIVE),
        _ => (0, 0, 1, 1),
    }
}

/// Draws one action button with its sprite (active variant while held).
pub fn draw_button(drawList: &mut GuiDrawList, widget: &TouchWidget, active: bool) {
    let (x, y, w, h) = rect_of(widget);
    let sprite = match widget {
        TouchWidget::Sneak { .. } => pick(active, SNEAK, SNEAK_ACTIVE),
        TouchWidget::Chat { .. } => pick(active, CHAT, CHAT_ACTIVE),
        TouchWidget::Pause { .. } => pick(active, ESC, ESC_ACTIVE),
        TouchWidget::Backpack { .. } => pick(active, BACKPACK, BACKPACK_ACTIVE),
        TouchWidget::Ascend { .. } => pick(active, ASCEND, ASCEND_ACTIVE),
        TouchWidget::Descend { .. } => pick(active, DESCEND, DESCEND_ACTIVE),
        TouchWidget::Jump { .. } => return, // drawn by draw_jump (flying state)
        TouchWidget::BackpackClose { .. } => pick(active, CROSS, CROSS_ACTIVE),
        // The credits skip maps the Escape key, so it reuses the ESC sprite.
        TouchWidget::WinGameSkip { .. } => pick(active, ESC, ESC_ACTIVE),
    };
    draw_sprite(drawList, x, y, w, h, sprite);
}

/// Jump button: jump sprite on the ground, flight sprite while flying (the
/// flight key is the jump key's flying-state texture).
pub fn draw_jump(drawList: &mut GuiDrawList, rect: (i32, i32, i32, i32), active: bool, flying: bool) {
    let sprite = if flying { pick(active, FLIGHT, FLIGHT_ACTIVE) } else { pick(active, JUMP, JUMP_ACTIVE) };
    draw_sprite(drawList, rect.0, rect.1, rect.2, rect.3, sprite);
}

#[cfg(test)]
mod tests {
    use super::button_rects;

    #[test]
    fn classic_rects_are_non_overlapping() {
        let (dpad, buttons) = button_rects(600, 270);
        for button in &buttons {
            // Rects are (x, y, width, height).
            let overlap = dpad.0 < button.0 + button.2 && dpad.0 + dpad.2 > button.0
                && dpad.1 < button.1 + button.3 && dpad.1 + dpad.3 > button.1;
            assert!(!overlap, "button overlaps dpad: {button:?}");
        }
    }
}
