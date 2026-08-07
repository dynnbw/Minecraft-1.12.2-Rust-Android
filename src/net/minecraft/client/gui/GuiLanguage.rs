use crate::net::minecraft::client::gui::FontRenderer::FontRenderer;
use crate::net::minecraft::client::gui::GuiButton::{GuiButton, GuiSoundCommand};
use crate::net::minecraft::client::gui::GuiScreen::GuiScreen;
use crate::net::minecraft::client::resources::Language::Language;
use crate::net::minecraft::client::resources::LanguageManager::LanguageManager;
use crate::net::minecraft::client::resources::Locale::Locale;
use crate::net::minecraft::client::settings::GameSettings::GameSettings;
use crate::vulkan::GuiDrawList::GuiDrawList;

#[derive(Debug, Clone, PartialEq)]
pub enum GuiLanguageAction { ToggleUnicode, Done, SelectLanguage(String) }
#[derive(Debug, Clone, PartialEq)]
pub struct GuiLanguageInteraction { pub action: GuiLanguageAction, pub sound: Option<GuiSoundCommand> }

/// MCP `GuiLanguage`. The Java class's inner `List` (a `GuiSlot`) is ported
/// inline: 18px rows between y=32 and height-65+4, so scrolling, slot
/// hit-testing and the wheel follow `GuiSlot` (`getMaxScroll` uses the
/// bottom-4 inset, one wheel notch scrolls half a slot).
#[derive(Debug, Clone)]
pub struct GuiLanguage {
    pub GuiScreen: GuiScreen,
    currentLanguage: String,
    languages: Vec<Language>,
    scrollOffset: i32,
}

const SLOT_HEIGHT: i32 = 18;
const LIST_TOP: i32 = 32;
const LIST_BOTTOM_INSET: i32 = 65;
/// MCP `GuiSlot#getListWidth` returns half the screen width by default.
const LIST_SIDE_INSET: i32 = 110;

impl GuiLanguage {
    pub fn new(currentLanguage: impl Into<String>) -> Self {
        Self {
            GuiScreen: GuiScreen::default(),
            currentLanguage: currentLanguage.into(),
            languages: Vec::new(),
            scrollOffset: 0,
        }
    }

    pub fn initGui(
        &mut self,
        width: i32,
        height: i32,
        locale: &Locale,
        settings: &GameSettings,
        languageManager: &LanguageManager,
    ) {
        self.GuiScreen.width = width;
        self.GuiScreen.height = height;
        self.GuiScreen.buttonList.clear();
        let unicodeLabel = format!(
            "{}: {}",
            locale.translate_key("options.forceUnicodeFont"),
            if settings.forceUnicodeFont { locale.translate_key("options.on") } else { locale.translate_key("options.off") }
        );
        self.GuiScreen.buttonList.push(GuiButton::newWithSize(100, width / 2 - 155, height - 38, 150, 20, unicodeLabel));
        self.GuiScreen.buttonList.push(GuiButton::newWithSize(6, width / 2 + 5, height - 38, 150, 20, locale.translate_key("gui.done")));
        // MCP `GuiLanguage.List` constructor: snapshot the sorted languages
        // and the current selection. The scroll offset survives re-inits so a
        // language switch does not jump the list (vanilla keeps the slot's
        // amountScrolled across elementClicked).
        self.languages = languageManager.getLanguages().into_iter().cloned().collect();
        self.currentLanguage = languageManager.getCurrentLanguage().getLanguageCode().to_owned();
    }

    fn listBottom(&self) -> i32 {
        self.GuiScreen.height - LIST_BOTTOM_INSET + 4
    }

    /// MCP `GuiSlot#getMaxScroll`: `max(0, contentHeight - (bottom - top - 4))`.
    fn maxScroll(&self) -> i32 {
        let contentHeight = self.languages.len() as i32 * SLOT_HEIGHT;
        (contentHeight - (self.listBottom() - LIST_TOP - 4)).max(0)
    }

    /// MCP `GuiSlot#handleMouseInput`: one wheel notch scrolls half a slot.
    pub fn scroll(&mut self, lines: f32) -> bool {
        let amount = (lines * SLOT_HEIGHT as f32 / 2.0) as i32;
        if amount == 0 { return false; }
        self.scrollOffset = (self.scrollOffset - amount).clamp(0, self.maxScroll());
        true
    }

    pub fn drawScreen(
        &mut self,
        drawList: &mut GuiDrawList,
        font: &mut FontRenderer,
        locale: &Locale,
        mouseX: i32,
        mouseY: i32,
        partialTicks: f32,
    ) {
        // MCP GuiLanguage.drawScreen: list first, then the title and the
        // (localized) warning, then the buttons through GuiScreen.drawScreen.
        let listLeft = self.GuiScreen.width / 2 - LIST_SIDE_INSET;
        let listRight = self.GuiScreen.width / 2 + LIST_SIDE_INSET;
        let first = (self.scrollOffset / SLOT_HEIGHT).max(0) as usize;
        let offsetWithin = self.scrollOffset.rem_euclid(SLOT_HEIGHT);
        for (index, language) in self.languages.iter().enumerate().skip(first) {
            let y = LIST_TOP - offsetWithin + (index as i32 - first as i32) * SLOT_HEIGHT;
            if y + SLOT_HEIGHT <= LIST_TOP || y >= self.listBottom() {
                continue;
            }
            // MCP `GuiSlot#func_192638_a` selection highlight plus
            // `func_192637_a` row text ("name (region)").
            if language.getLanguageCode() == self.currentLanguage {
                drawList.draw_rect(listLeft, y, listRight, y + SLOT_HEIGHT, 0x8060_6060_u32 as i32);
            }
            self.GuiScreen.Gui.drawCenteredString(font, drawList, &language.to_string(), self.GuiScreen.width / 2, y + 1, 0x00FF_FFFF);
        }
        self.GuiScreen.Gui.drawCenteredString(font, drawList, locale.translate_key("options.language"), self.GuiScreen.width / 2, 16, 0x00FF_FFFF);
        let warning = format!("({})", locale.translate_key("options.languageWarning"));
        self.GuiScreen.Gui.drawCenteredString(font, drawList, &warning, self.GuiScreen.width / 2, self.GuiScreen.height - 56, 0x0080_8080);
        self.GuiScreen.drawScreen(drawList, font, mouseX, mouseY, partialTicks);
    }

    pub fn mouseClicked(&mut self, mouseX: i32, mouseY: i32, mouseButton: i32) -> Option<GuiLanguageInteraction> {
        if mouseButton != 0 { return None; }
        // MCP `GuiSlot#mouseClicked`: hit a row only when the Y is within
        // [top, bottom] and the X within the list bounds; the slot index is
        // `(y - top + amountScrolled - 4) / slotHeight` (the bottom-4 inset).
        let listLeft = self.GuiScreen.width / 2 - LIST_SIDE_INSET;
        let listRight = self.GuiScreen.width / 2 + LIST_SIDE_INSET;
        if mouseX >= listLeft && mouseX <= listRight && mouseY >= LIST_TOP && mouseY <= self.listBottom() {
            let index = ((mouseY - LIST_TOP + self.scrollOffset - 4) / SLOT_HEIGHT).max(0) as usize;
            if let Some(language) = self.languages.get(index) {
                // GuiSlot#elementClicked plays no sound.
                return Some(GuiLanguageInteraction {
                    action: GuiLanguageAction::SelectLanguage(language.getLanguageCode().to_owned()),
                    sound: None,
                });
            }
        }
        self.GuiScreen.buttonList.iter().find_map(|button| {
            if !button.mousePressed(mouseX, mouseY) { return None; }
            let action = match button.id {
                100 => GuiLanguageAction::ToggleUnicode,
                6 => GuiLanguageAction::Done,
                _ => return None,
            };
            Some(GuiLanguageInteraction { action, sound: Some(button.playPressSound()) })
        })
    }
}
