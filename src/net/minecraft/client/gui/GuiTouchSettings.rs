use crate::net::minecraft::client::gui::FontRenderer::FontRenderer;
use crate::net::minecraft::client::gui::GuiButton::{GuiButton, GuiSoundCommand};
use crate::net::minecraft::client::gui::GuiScreen::GuiScreen;
use crate::net::minecraft::client::resources::Locale::Locale;
use crate::net::minecraft::client::settings::GameSettings::GameSettings;
use crate::vulkan::GuiDrawList::GuiDrawList;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiTouchSettingsAction {
    ToggleEnabled,
    ResetDefaults,
    Done,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiTouchSettingsInteraction {
    pub action: GuiTouchSettingsAction,
    pub sound: Option<GuiSoundCommand>,
}

/// Bedrock-style touch layer settings (Android): enable/disable the layer,
/// reset to defaults and Done. Toggling persists to options.txt through the
/// `touchEnabled` key; the button text mirrors the current setting.
#[derive(Debug, Clone)]
pub struct GuiTouchSettings {
    pub GuiScreen: GuiScreen,
    title: String,
    enabled: GuiButton,
    reset: GuiButton,
    done: GuiButton,
}

impl Default for GuiTouchSettings {
    fn default() -> Self {
        Self {
            GuiScreen: GuiScreen::default(),
            title: "Touch Controls".to_owned(),
            enabled: GuiButton::newWithSize(111, 0, 0, 150, 20, ""),
            reset: GuiButton::newWithSize(112, 0, 0, 150, 20, ""),
            done: GuiButton::new(200, 0, 0, ""),
        }
    }
}

impl GuiTouchSettings {
    pub fn new() -> Self { Self::default() }

    pub fn initGui(&mut self, width: i32, height: i32, locale: &Locale, settings: &GameSettings) {
        self.GuiScreen.width = width;
        self.GuiScreen.height = height;
        self.enabled.x = width / 2 - 155;
        self.enabled.y = height / 6 - 12;
        self.enabled.displayString = enabled_label(locale, settings.touch.enabled);
        self.reset.x = width / 2 - 155;
        self.reset.y = height / 6 + 24;
        self.reset.displayString = "Reset to Defaults".to_owned();
        self.done.x = width / 2 - 100;
        self.done.y = height / 6 + 168;
        self.done.displayString = locale.translate_key("gui.done").to_owned();
    }

    pub fn drawScreen(&mut self, draw: &mut GuiDrawList, font: &mut FontRenderer, mouseX: i32, mouseY: i32, partial: f32) {
        self.draw(draw, font, mouseX, mouseY, partial, false);
    }
    pub fn drawScreenInWorld(&mut self, draw: &mut GuiDrawList, font: &mut FontRenderer, mouseX: i32, mouseY: i32, partial: f32) {
        self.draw(draw, font, mouseX, mouseY, partial, true);
    }
    fn draw(&mut self, draw: &mut GuiDrawList, font: &mut FontRenderer, mouseX: i32, mouseY: i32, partial: f32, world: bool) {
        if world { self.GuiScreen.drawDefaultBackgroundInWorld(draw); } else { self.GuiScreen.drawDefaultBackground(draw); }
        self.GuiScreen.Gui.drawCenteredString(font, draw, &self.title, self.GuiScreen.width / 2, 15, 0x00FF_FFFF);
        self.enabled.drawButton(draw, font, mouseX, mouseY, partial);
        self.reset.drawButton(draw, font, mouseX, mouseY, partial);
        self.done.drawButton(draw, font, mouseX, mouseY, partial);
    }

    pub fn mouseClicked(&mut self, x: i32, y: i32, button: i32, locale: &Locale, settings: &GameSettings) -> Option<GuiTouchSettingsInteraction> {
        if button != 0 { return None; }
        if self.enabled.mousePressed(x, y) {
            self.enabled.displayString = enabled_label(locale, !settings.touch.enabled);
            return Some(GuiTouchSettingsInteraction { action: GuiTouchSettingsAction::ToggleEnabled, sound: Some(self.enabled.playPressSound()) });
        }
        if self.reset.mousePressed(x, y) {
            return Some(GuiTouchSettingsInteraction { action: GuiTouchSettingsAction::ResetDefaults, sound: Some(self.reset.playPressSound()) });
        }
        self.done.mousePressed(x, y).then(|| GuiTouchSettingsInteraction { action: GuiTouchSettingsAction::Done, sound: Some(self.done.playPressSound()) })
    }
}

fn enabled_label(locale: &Locale, enabled: bool) -> String {
    let state = locale.translate_key(if enabled { "gui.yes" } else { "gui.no" });
    format!("Enabled: {state}")
}
