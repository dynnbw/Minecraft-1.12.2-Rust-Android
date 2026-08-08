use crate::compat::Java::JavaRandom;
use crate::net::minecraft::client::gui::FontRenderer::FontRenderer;
use crate::net::minecraft::client::gui::GuiScreen::GuiScreen;
use crate::net::minecraft::client::resources::SimpleReloadableResourceManager::ResourceManager;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;
use crate::vulkan::GuiDrawList::GuiDrawList;

const END_TEXT: &str = "texts/end.txt";
const CREDITS_TEXT: &str = "texts/credits.txt";
const MINECRAFT_LOGO: &str = "textures/gui/title/minecraft.png";
const EDITION_TEXTURE: &str = "textures/gui/title/edition.png";
const OPTIONS_BACKGROUND: &str = "textures/gui/options_background.png";
/// `GuiWinGame`: `"" + WHITE + OBFUSCATED + GREEN + AQUA`, the marker replaced
/// with a randomized obfuscated run of `X`s.
const OBFUSCATED_MARKER: &str = "\u{a7}f\u{a7}k\u{a7}a\u{a7}b";

/// Direct MCP 1.12.2 `GuiWinGame` port: the end-credits scroll. When the
/// scroll runs out (`updateScreen`) or Escape is pressed, `finished` reports
/// true and the caller sends `CPacketClientStatus(PERFORM_RESPAWN)`.
#[derive(Debug, Clone)]
pub struct GuiWinGame {
    pub GuiScreen: GuiScreen,
    time: f32,
    lines: Vec<String>,
    totalScrollLength: i32,
    scrollSpeed: f32,
    showEndText: bool,
    finished: bool,
}

impl GuiWinGame {
    pub fn new(showEndText: bool) -> Self {
        Self {
            GuiScreen: GuiScreen::default(),
            time: 0.0,
            lines: Vec::new(),
            totalScrollLength: 0,
            scrollSpeed: if showEndText { 0.5 } else { 0.75 },
            showEndText,
            finished: false,
        }
    }

    /// `GuiWinGame#initGui`: lazily loads `texts/end.txt` (first credits run
    /// only) and `texts/credits.txt`, wrapping each line to 274 pixels.
    pub fn initGui(
        &mut self,
        width: i32,
        height: i32,
        resources: &ResourceManager,
        username: &str,
        font: &mut FontRenderer,
    ) {
        self.GuiScreen.width = width;
        self.GuiScreen.height = height;
        if !self.lines.is_empty() {
            return;
        }
        let mut lines = Vec::new();
        if self.showEndText {
            if let Ok(resource) = resources.get_resource(&ResourceLocation::parse(END_TEXT)) {
                let mut random = JavaRandom::new(8_124_371);
                for line in String::from_utf8_lossy(&resource.bytes).lines() {
                    let mut line = line.replace("PLAYERNAME", username);
                    while line.contains(OBFUSCATED_MARKER) {
                        let run = "XXXXXXXX";
                        let length = 3 + random.next_i32_bound(4) as usize;
                        let replacement = format!("\u{a7}f\u{a7}k{}", &run[..length]);
                        line = line.replacen(OBFUSCATED_MARKER, &replacement, 1);
                    }
                    lines.extend(font.list_formatted_string_to_width(&line, 274));
                    lines.push(String::new());
                }
                lines.extend(std::iter::repeat(String::new()).take(8));
            }
        }
        if let Ok(resource) = resources.get_resource(&ResourceLocation::parse(CREDITS_TEXT)) {
            for line in String::from_utf8_lossy(&resource.bytes).lines() {
                let line = line.replace("PLAYERNAME", username).replace('\t', "    ");
                lines.extend(font.list_formatted_string_to_width(&line, 274));
                lines.push(String::new());
            }
        }
        self.lines = lines;
        self.totalScrollLength = self.lines.len() as i32 * 12;
    }

    /// `GuiWinGame#updateScreen`: once the scrolled time passes the total
    /// scroll duration, the respawn Runnable fires.
    pub fn updateScreen(&mut self) -> bool {
        let f = (self.totalScrollLength + self.GuiScreen.height * 2 + 24) as f32 / self.scrollSpeed;
        if self.time > f {
            self.finished = true;
        }
        self.finished
    }

    /// `GuiWinGame#keyTyped` with keyCode 1 (Escape).
    pub fn keyPressedEscape(&mut self) -> bool {
        self.finished = true;
        true
    }

    pub const fn isFinished(&self) -> bool {
        self.finished
    }

    /// `GuiWinGame#drawWinGameScreen`: the scrolling options-background with
    /// the MCP fade-in color ramp.
    fn drawWinGameScreen(&self, drawList: &mut GuiDrawList) {
        let f = -self.time * 0.5 * self.scrollSpeed;
        let f1 = self.GuiScreen.height as f32 - self.time * 0.5 * self.scrollSpeed;
        let mut f3 = self.time * 0.02;
        let f4 =
            (self.totalScrollLength + self.GuiScreen.height * 2 + 24) as f32 / self.scrollSpeed;
        let f5 = (f4 - 20.0 - self.time) * 0.005;
        if f5 < f3 {
            f3 = f5;
        }
        if f3 > 1.0 {
            f3 = 1.0;
        }
        let f3 = (f3 * f3 * 96.0 / 255.0).min(1.0);
        let grey = (f3 * 255.0).round() as u32;
        let color = 0xFF00_0000 | (grey << 16) | (grey << 8) | grey;
        let width = self.GuiScreen.width as f32;
        let height = self.GuiScreen.height as f32;
        let uv = 1.0 / 64.0;
        drawList.push_textured_quad(
            ResourceLocation::parse(OPTIONS_BACKGROUND),
            [
                (0.0, height, 0.0, f * uv, color),
                (width, height, width * uv, f * uv, color),
                (width, 0.0, width * uv, f1 * uv, color),
                (0.0, 0.0, 0.0, f1 * uv, color),
            ],
        );
    }

    pub fn drawScreen(
        &mut self,
        drawList: &mut GuiDrawList,
        font: &mut FontRenderer,
        mouseX: i32,
        mouseY: i32,
        partialTicks: f32,
    ) {
        self.drawWinGameScreen(drawList);
        self.time += partialTicks;
        let f = -self.time * self.scrollSpeed;
        let width = self.GuiScreen.width;
        let height = self.GuiScreen.height;
        let j = width / 2 - 137;
        let k = height + 50;
        drawList.push_matrix();
        drawList.translate(0.0, f);
        drawList.draw_textured_modal_rect(
            ResourceLocation::parse(MINECRAFT_LOGO),
            j,
            k,
            0,
            0,
            155,
            44,
        );
        drawList.draw_textured_modal_rect(
            ResourceLocation::parse(MINECRAFT_LOGO),
            j + 155,
            k,
            0,
            45,
            155,
            44,
        );
        drawList.draw_modal_rect_with_custom_sized_texture(
            ResourceLocation::parse(EDITION_TEXTURE),
            (j + 88) as f32,
            (k + 37) as f32,
            0.0,
            0.0,
            98.0,
            14.0,
            128.0,
            16.0,
        );
        let mut lineY = k + 100;
        for (index, s) in self.lines.iter().enumerate() {
            if index == self.lines.len() - 1 {
                let f1 = lineY as f32 + f - (height as f32 / 2.0 - 6.0);
                if f1 < 0.0 {
                    drawList.translate(0.0, -f1);
                }
            }
            if lineY as f32 + f + 20.0 > 0.0 && lineY as f32 + f < height as f32 {
                if let Some(text) = s.strip_prefix("[C]") {
                    font.draw_centered_string_with_shadow(
                        drawList,
                        text,
                        j + (274 - font.get_string_width(text)) / 2,
                        lineY,
                        0x00FF_FFFF,
                    );
                } else {
                    font.draw_string_with_shadow(drawList, s, j as f32, lineY as f32, 0x00FF_FFFF);
                }
            }
            lineY += 12;
        }
        drawList.pop_matrix();
        // The final vignette pass (blendFunc ZERO/ONE_MINUS_SRC_COLOR) has no
        // GuiDrawList equivalent; the background fade covers the look-in.
        let _ = (mouseX, mouseY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_scrolls_half_speed_then_replay_three_quarters() {
        assert_eq!(GuiWinGame::new(true).scrollSpeed, 0.5);
        assert_eq!(GuiWinGame::new(false).scrollSpeed, 0.75);
    }

    #[test]
    fn empty_credits_finish_after_full_scroll_duration() {
        let mut screen = GuiWinGame::new(false);
        screen.GuiScreen.height = 480;
        screen.totalScrollLength = 0;
        screen.time = 0.0;
        assert!(!screen.updateScreen());
        // (0 + 480 + 480 + 24) / 0.75 = 1312 ticks; `time > f` is strict.
        screen.time = 1312.0;
        assert!(!screen.updateScreen());
        screen.time = 1312.1;
        assert!(screen.updateScreen());
    }

    #[test]
    fn escape_requests_finish_immediately() {
        let mut screen = GuiWinGame::new(true);
        assert!(screen.keyPressedEscape());
        assert!(screen.isFinished());
    }
}
