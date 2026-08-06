use std::path::Path;

use anyhow::Context;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::launcher::RenderBackend::RenderBackend;
use crate::net::minecraft::client::settings::GameSettings::GameSettings;
#[cfg(not(target_os = "android"))]
use crate::opengl::OpenGlWindow::OpenGlWindow;
use crate::vulkan::CpuFrame::CpuFrame;
use crate::vulkan::GuiRenderFrame::GuiRenderFrame;
use crate::vulkan::VulkanWindow::VulkanWindow;
use crate::vulkan::VulkanWorldRenderer::WorldRenderFrame;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RendererExtent {
    pub width: u32,
    pub height: u32,
}

/// Native presentation backend selected before window creation. Minecraft and
/// MCP renderer classes prepare the same semantic frame for either API; only
/// native resource ownership and draw submission differ here.
pub enum DesktopRenderer {
    Vulkan(VulkanWindow),
    #[cfg(not(target_os = "android"))]
    OpenGl(OpenGlWindow),
}

impl DesktopRenderer {
    pub fn create(
        eventLoop: &ActiveEventLoop,
        attributes: WindowAttributes,
        gameSettings: &GameSettings,
        _gameDir: &Path,
    ) -> anyhow::Result<(Window, Self)> {
        match gameSettings.renderBackend {
            RenderBackend::Vulkan => {
                let window = eventLoop
                    .create_window(attributes)
                    .context("failed creating Minecraft Vulkan window")?;
                let renderer = VulkanWindow::new(&window, gameSettings)
                    .context("failed initializing Minecraft Vulkan output")?;
                Ok((window, Self::Vulkan(renderer)))
            }
            #[cfg(not(target_os = "android"))]
            RenderBackend::OpenGl => {
                let (window, renderer) = OpenGlWindow::create(eventLoop, attributes, gameSettings, _gameDir)
                    .context("failed initializing Minecraft OpenGL output")?;
                Ok((window, Self::OpenGl(renderer)))
            }
            #[cfg(target_os = "android")]
            RenderBackend::OpenGl => unreachable!("Android builds force the Vulkan backend"),
        }
    }

    pub const fn backend(&self) -> RenderBackend {
        match self {
            Self::Vulkan(_) => RenderBackend::Vulkan,
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(_) => RenderBackend::OpenGl,
        }
    }

    pub fn extent(&self) -> RendererExtent {
        match self {
            Self::Vulkan(renderer) => {
                let extent = renderer.extent();
                RendererExtent { width: extent.width, height: extent.height }
            }
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.extent(),
        }
    }

    pub fn deviceName(&self) -> &str {
        match self {
            Self::Vulkan(renderer) => renderer.deviceName(),
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.deviceName(),
        }
    }

    pub fn drawFrame(&mut self, window: &Window, frame: &CpuFrame) -> anyhow::Result<()> {
        match self {
            Self::Vulkan(renderer) => renderer.drawFrame(window, frame),
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.drawFrame(window, frame),
        }
    }

    pub fn drawNativeGuiFrame(&mut self, window: &Window, frame: &GuiRenderFrame) -> anyhow::Result<()> {
        match self {
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.drawNativeGuiFrame(window, frame),
            Self::Vulkan(renderer) => renderer.drawNativeGuiFrame(window, frame),
        }
    }

    pub fn drawWorldFrame(&mut self, window: &Window, frame: &WorldRenderFrame) -> anyhow::Result<()> {
        match self {
            Self::Vulkan(renderer) => renderer.drawWorldFrame(window, frame),
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.drawWorldFrame(window, frame),
        }
    }

    pub fn resize(&mut self, window: &Window) -> anyhow::Result<()> {
        match self {
            Self::Vulkan(renderer) => renderer.resize(window),
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.resize(window),
        }
    }

    pub fn setVsync(&mut self, window: &Window, enableVsync: bool) -> anyhow::Result<()> {
        match self {
            Self::Vulkan(renderer) => renderer.setVsync(window, enableVsync),
            #[cfg(not(target_os = "android"))]
            Self::OpenGl(renderer) => renderer.setVsync(enableVsync),
        }
    }
    #[cfg(not(target_os = "android"))]
    pub fn reloadShaderPack(&mut self) {
        if let Self::OpenGl(renderer) = self {
            renderer.reloadShaderPack();
        }
    }

}
