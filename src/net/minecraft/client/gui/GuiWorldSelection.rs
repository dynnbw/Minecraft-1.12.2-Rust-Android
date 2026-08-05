use crate::net::minecraft::client::gui::FontRenderer::FontRenderer;
use crate::net::minecraft::client::gui::GuiButton::{GuiButton, GuiSoundCommand};
use crate::net::minecraft::client::gui::GuiScreen::GuiScreen;
use crate::net::minecraft::client::resources::Locale::Locale;
use crate::vulkan::GuiDrawList::GuiDrawList;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiWorldSelectionAction { Select, Create, Edit, Delete, Recreate, Cancel }
#[derive(Debug, Clone, PartialEq)]
pub struct GuiWorldSelectionInteraction { pub action: GuiWorldSelectionAction, pub sound: GuiSoundCommand }

#[derive(Debug, Clone)]
pub struct GuiWorldSelection { pub GuiScreen: GuiScreen, pub title: String }

impl Default for GuiWorldSelection {
    fn default() -> Self { Self { GuiScreen: GuiScreen::default(), title: "Select world".to_owned() } }
}

impl GuiWorldSelection {
    pub fn new() -> Self { Self::default() }

    pub fn initGui(&mut self, width: i32, height: i32, locale: &Locale) {
        self.GuiScreen.width = width;
        self.GuiScreen.height = height;
        self.GuiScreen.buttonList.clear();
        self.title = locale.translate_key("selectWorld.title").to_owned();
        let mut select = GuiButton::newWithSize(1, width / 2 - 154, height - 52, 150, 20, locale.translate_key("selectWorld.select"));
        select.enabled = false;
        self.GuiScreen.buttonList.push(select);
        self.GuiScreen.buttonList.push(GuiButton::newWithSize(3, width / 2 + 4, height - 52, 150, 20, locale.translate_key("selectWorld.create")));
        for (id, x, key) in [
            (4, width / 2 - 154, "selectWorld.edit"),
            (2, width / 2 - 76, "selectWorld.delete"),
            (5, width / 2 + 4, "selectWorld.recreate"),
            (0, width / 2 + 82, "gui.cancel"),
        ] {
            let mut button = GuiButton::newWithSize(id, x, height - 28, 72, 20, locale.translate_key(key));
            if id != 0 { button.enabled = false; }
            self.GuiScreen.buttonList.push(button);
        }
    }

    pub fn drawScreen(&mut self, drawList: &mut GuiDrawList, font: &mut FontRenderer, mouseX: i32, mouseY: i32, partialTicks: f32) {
        self.GuiScreen.drawDefaultBackground(drawList);
        self.GuiScreen.Gui.drawCenteredString(font, drawList, &self.title, self.GuiScreen.width / 2, 20, 0x00FF_FFFF);
        self.GuiScreen.drawScreen(drawList, font, mouseX, mouseY, partialTicks);
    }

    pub fn mouseClicked(&self, mouseX: i32, mouseY: i32, mouseButton: i32) -> Option<GuiWorldSelectionInteraction> {
        if mouseButton != 0 { return None; }
        self.GuiScreen.buttonList.iter().find_map(|button| {
            if !button.mousePressed(mouseX, mouseY) { return None; }
            let action = match button.id {
                1 => GuiWorldSelectionAction::Select,
                3 => GuiWorldSelectionAction::Create,
                4 => GuiWorldSelectionAction::Edit,
                2 => GuiWorldSelectionAction::Delete,
                5 => GuiWorldSelectionAction::Recreate,
                0 => GuiWorldSelectionAction::Cancel,
                _ => return None,
            };
            Some(GuiWorldSelectionInteraction { action, sound: button.playPressSound() })
        })
    }
}
