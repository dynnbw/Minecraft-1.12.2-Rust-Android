use crate::net::minecraft::client::gui::FontRenderer::FontRenderer;
use crate::net::minecraft::client::gui::GuiButton::{GuiButton, GuiSoundCommand};
use crate::net::minecraft::client::gui::GuiScreen::GuiScreen;
use crate::net::minecraft::client::resources::Locale::Locale;
use crate::net::minecraft::client::settings::GameSettings::GameSettings;
use crate::vulkan::GuiDrawList::GuiDrawList;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiLanguageAction { ToggleUnicode, Done }
#[derive(Debug, Clone, PartialEq)]
pub struct GuiLanguageInteraction { pub action: GuiLanguageAction, pub sound: GuiSoundCommand }

#[derive(Debug, Clone)]
pub struct GuiLanguage { pub GuiScreen: GuiScreen, currentLanguage: String }

impl GuiLanguage {
    pub fn new(currentLanguage: impl Into<String>) -> Self {
        Self { GuiScreen: GuiScreen::default(), currentLanguage: currentLanguage.into() }
    }

    pub fn initGui(&mut self, width: i32, height: i32, locale: &Locale, settings: &GameSettings) {
        self.GuiScreen.width = width;
        self.GuiScreen.height = height;
        self.GuiScreen.buttonList.clear();
        let unicodeLabel = format!("{}: {}", locale.translate_key("options.forceUnicodeFont"), if settings.forceUnicodeFont { locale.translate_key("options.on") } else { locale.translate_key("options.off") });
        self.GuiScreen.buttonList.push(GuiButton::newWithSize(100, width / 2 - 155, height - 38, 150, 20, unicodeLabel));
        self.GuiScreen.buttonList.push(GuiButton::newWithSize(6, width / 2 + 5, height - 38, 150, 20, locale.translate_key("gui.done")));
    }

    pub fn drawScreen(&mut self, drawList: &mut GuiDrawList, font: &mut FontRenderer, locale: &Locale, mouseX: i32, mouseY: i32, partialTicks: f32) {
        self.GuiScreen.drawDefaultBackground(drawList);
        self.GuiScreen.Gui.drawCenteredString(font, drawList, locale.translate_key("options.language"), self.GuiScreen.width / 2, 16, 0x00FF_FFFF);
        self.GuiScreen.Gui.drawCenteredString(font, drawList, &self.currentLanguage, self.GuiScreen.width / 2, 44, 0x00FF_FFFF);
        let warning = format!("({})", locale.translate_key("options.languageWarning"));
        self.GuiScreen.Gui.drawCenteredString(font, drawList, &warning, self.GuiScreen.width / 2, self.GuiScreen.height - 56, 0x0080_8080);
        self.GuiScreen.drawScreen(drawList, font, mouseX, mouseY, partialTicks);
    }

    pub fn mouseClicked(&self, mouseX: i32, mouseY: i32, mouseButton: i32) -> Option<GuiLanguageInteraction> {
        if mouseButton != 0 { return None; }
        self.GuiScreen.buttonList.iter().find_map(|button| {
            if !button.mousePressed(mouseX, mouseY) { return None; }
            let action = match button.id { 100 => GuiLanguageAction::ToggleUnicode, 6 => GuiLanguageAction::Done, _ => return None };
            Some(GuiLanguageInteraction { action, sound: button.playPressSound() })
        })
    }
}
