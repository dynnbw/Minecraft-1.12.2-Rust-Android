use std::collections::BTreeSet;
use std::ffi::{CStr, CString};
use std::ptr::NonNull;

use anyhow::{anyhow, Context};
use ash::{khr, vk, Device, Entry, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

use crate::net::minecraft::client::settings::GameSettings::GameSettings;
use crate::vulkan::CpuFrame::CpuFrame;
use crate::vulkan::GuiRenderFrame::GuiRenderFrame;
use crate::vulkan::VulkanGuiPipeline::VulkanGuiPipeline;
use crate::vulkan::SwapchainPolicy::choose_swapchain;
use crate::vulkan::VulkanWorldRenderer::WorldRenderFrame;
use crate::vulkan::WorldGpuPipeline::WorldGpuPipeline;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct QueueFamilies {
    graphics: u32,
    present: u32,
}

struct FrameSynchronization {
    imageAvailableSemaphore: vk::Semaphore,
    renderFinishedSemaphore: vk::Semaphore,
    inFlightFence: vk::Fence,
}

struct UploadBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped: NonNull<u8>,
    size: vk::DeviceSize,
}

/// Window-bound Vulkan objects. This remains outside the MCP package tree:
/// Mojang's classes generate the exact menu commands, while this object owns
/// only native Vulkan presentation resources.
pub struct VulkanWindow {
    entry: Entry,
    instance: Instance,
    surfaceLoader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physicalDevice: vk::PhysicalDevice,
    physicalDeviceName: String,
    memoryProperties: vk::PhysicalDeviceMemoryProperties,
    device: Device,
    graphicsQueue: vk::Queue,
    presentQueue: vk::Queue,
    queueFamilies: QueueFamilies,
    swapchainLoader: khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchainFormat: vk::Format,
    swapchainExtent: vk::Extent2D,
    swapchainImages: Vec<vk::Image>,
    commandPool: vk::CommandPool,
    commandBuffers: Vec<vk::CommandBuffer>,
    uploadBuffers: Vec<UploadBuffer>,
    imageInitialized: Vec<bool>,
    synchronization: Vec<FrameSynchronization>,
    imagesInFlight: Vec<vk::Fence>,
    currentFrame: usize,
    enableVsync: bool,
    worldPipeline: Option<WorldGpuPipeline>,
    guiPipeline: Option<VulkanGuiPipeline>,
    wideLinesEnabled: bool,
    multiDrawIndirectEnabled: bool,
}

impl VulkanWindow {
    pub fn new(window: &Window, gameSettings: &GameSettings) -> anyhow::Result<Self> {
        #[cfg(target_os = "android")]
        let entry = unsafe {
            // Android has no statically linkable Vulkan loader; dlopen
            // libvulkan.so (always present on Android 12+ devices).
            ash::Entry::load().context("failed loading libvulkan.so")?
        };
        #[cfg(not(target_os = "android"))]
        let entry = Entry::linked();
        let displayHandle = window
            .display_handle()
            .context("failed to obtain the native display handle")?
            .as_raw();
        let windowHandle = window
            .window_handle()
            .context("failed to obtain the native window handle")?
            .as_raw();

        let applicationName = CString::new("Minecraft 1.12.2").expect("static CString");
        let engineName = CString::new("Minecraft 1.12.2 Vulkan Renderer").expect("static CString");
        let applicationInfo = vk::ApplicationInfo::default()
            .application_name(&applicationName)
            .application_version(vk::make_api_version(0, 1, 12, 2))
            .engine_name(&engineName)
            .engine_version(vk::make_api_version(0, 0, 4, 0))
            .api_version(vk::API_VERSION_1_1);

        let requiredExtensions = ash_window::enumerate_required_extensions(displayHandle)
            .context("the windowing backend did not expose required Vulkan extensions")?;
        let instanceCreateInfo = vk::InstanceCreateInfo::default()
            .application_info(&applicationInfo)
            .enabled_extension_names(requiredExtensions);
        let instance = unsafe { entry.create_instance(&instanceCreateInfo, None) }
            .context("failed to create Vulkan instance")?;

        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, displayHandle, windowHandle, None)
        }
        .context("failed to create Vulkan window surface")?;
        let surfaceLoader = khr::surface::Instance::new(&entry, &instance);

        let (physicalDevice, queueFamilies) = unsafe {
            selectPhysicalDevice(&instance, &surfaceLoader, surface)
        }
        .context("no Vulkan device supports both graphics and presentation")?;
        let memoryProperties = unsafe { instance.get_physical_device_memory_properties(physicalDevice) };
        let supportedFeatures = unsafe { instance.get_physical_device_features(physicalDevice) };
        let wideLinesEnabled = supportedFeatures.wide_lines == vk::TRUE;
        let multiDrawIndirectEnabled = supportedFeatures.multi_draw_indirect == vk::TRUE;
        let physicalDeviceProperties = unsafe { instance.get_physical_device_properties(physicalDevice) };
        let physicalDeviceName = unsafe { CStr::from_ptr(physicalDeviceProperties.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        log::info!(
            "Vulkan output device: {} ({:?}), multi_draw_indirect={}",
            physicalDeviceName,
            physicalDeviceProperties.device_type,
            multiDrawIndirectEnabled,
        );
        if !wideLinesEnabled {
            log::warn!(
                "Vulkan wideLines is unavailable; block selection outlines use the required geometry and blend/depth state with a 1px device line instead of Minecraft 1.12.2's requested 2px width"
            );
        }

        let priorities = [1.0_f32];
        let uniqueFamilies = BTreeSet::from([queueFamilies.graphics, queueFamilies.present]);
        let queueCreateInfos = uniqueFamilies
            .iter()
            .map(|&family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&priorities)
            })
            .collect::<Vec<_>>();
        let deviceExtensions = [khr::swapchain::NAME.as_ptr()];
        let enabledFeatures = vk::PhysicalDeviceFeatures::default()
            .wide_lines(wideLinesEnabled)
            .multi_draw_indirect(multiDrawIndirectEnabled);
        let deviceCreateInfo = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queueCreateInfos)
            .enabled_extension_names(&deviceExtensions)
            .enabled_features(&enabledFeatures);
        let device = unsafe { instance.create_device(physicalDevice, &deviceCreateInfo, None) }
            .context("failed to create Vulkan logical device")?;
        let graphicsQueue = unsafe { device.get_device_queue(queueFamilies.graphics, 0) };
        let presentQueue = unsafe { device.get_device_queue(queueFamilies.present, 0) };
        let swapchainLoader = khr::swapchain::Device::new(&instance, &device);

        let commandPoolInfo = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queueFamilies.graphics)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let commandPool = unsafe { device.create_command_pool(&commandPoolInfo, None) }
            .context("failed to create Vulkan command pool")?;

        let mut renderer = Self {
            entry,
            instance,
            surfaceLoader,
            surface,
            physicalDevice,
            physicalDeviceName,
            memoryProperties,
            device,
            graphicsQueue,
            presentQueue,
            queueFamilies,
            swapchainLoader,
            swapchain: vk::SwapchainKHR::null(),
            swapchainFormat: vk::Format::UNDEFINED,
            swapchainExtent: vk::Extent2D { width: 0, height: 0 },
            swapchainImages: Vec::new(),
            commandPool,
            commandBuffers: Vec::new(),
            uploadBuffers: Vec::new(),
            imageInitialized: Vec::new(),
            synchronization: Vec::new(),
            imagesInFlight: Vec::new(),
            currentFrame: 0,
            enableVsync: gameSettings.enableVsync,
            worldPipeline: None,
            guiPipeline: None,
            wideLinesEnabled,
            multiDrawIndirectEnabled,
        };
        renderer.createSwapchainObjects(window)?;
        renderer.ensureWorldPipeline()?;
        renderer.ensureGuiPipeline()?;
        renderer.createSynchronizationObjects()?;
        Ok(renderer)
    }

    pub const fn extent(&self) -> vk::Extent2D { self.swapchainExtent }
    pub fn deviceName(&self) -> &str { &self.physicalDeviceName }

    pub fn drawFrame(&mut self, window: &Window, framePixels: &CpuFrame) -> anyhow::Result<()> {
        if self.swapchain == vk::SwapchainKHR::null() {
            self.recreateSwapchain(window)?;
            if self.swapchain == vk::SwapchainKHR::null() {
                return Ok(());
            }
        }
        anyhow::ensure!(
            framePixels.width() == self.swapchainExtent.width
                && framePixels.height() == self.swapchainExtent.height,
            "CPU GUI frame {}x{} does not match Vulkan swapchain {}x{}",
            framePixels.width(),
            framePixels.height(),
            self.swapchainExtent.width,
            self.swapchainExtent.height,
        );

        // Copy the Vulkan handles out of the frame slot before calling methods
        // that require `&mut self`. Holding a reference into `self.synchronization`
        // across `uploadFrame`/`recordUploadCommandBuffer` violates Rust's aliasing
        // rules even though Vulkan handles themselves are plain copyable values.
        let (imageAvailableSemaphore, renderFinishedSemaphore, inFlightFence) = {
            let frame = &self.synchronization[self.currentFrame];
            (
                frame.imageAvailableSemaphore,
                frame.renderFinishedSemaphore,
                frame.inFlightFence,
            )
        };
        unsafe {
            self.device
                .wait_for_fences(&[inFlightFence], true, u64::MAX)
                .context("failed waiting for Vulkan frame fence")?;
        }

        let acquired = unsafe {
            self.swapchainLoader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                imageAvailableSemaphore,
                vk::Fence::null(),
            )
        };
        let (imageIndex, acquireSuboptimal) = match acquired {
            Ok(value) => value,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                log::debug!("swapchain OUT_OF_DATE on acquire; recreating");
                self.recreateSwapchain(window)?;
                return Ok(());
            }
            Err(error) => return Err(anyhow!("failed to acquire swapchain image: {error:?}")),
        };

        let imageFence = self.imagesInFlight[imageIndex as usize];
        if imageFence != vk::Fence::null() {
            unsafe {
                self.device
                    .wait_for_fences(&[imageFence], true, u64::MAX)
                    .context("failed waiting for swapchain image fence")?;
            }
        }
        self.imagesInFlight[imageIndex as usize] = inFlightFence;

        self.uploadFrame(imageIndex as usize, framePixels)?;
        self.recordUploadCommandBuffer(imageIndex as usize)?;

        unsafe {
            self.device
                .reset_fences(&[inFlightFence])
                .context("failed resetting Vulkan frame fence")?;
        }
        let waitSemaphores = [imageAvailableSemaphore];
        let waitStages = [vk::PipelineStageFlags::TRANSFER];
        let commandBuffers = [self.commandBuffers[imageIndex as usize]];
        let signalSemaphores = [renderFinishedSemaphore];
        let submitInfo = vk::SubmitInfo::default()
            .wait_semaphores(&waitSemaphores)
            .wait_dst_stage_mask(&waitStages)
            .command_buffers(&commandBuffers)
            .signal_semaphores(&signalSemaphores);
        unsafe {
            self.device
                .queue_submit(self.graphicsQueue, &[submitInfo], inFlightFence)
                .context("failed submitting Vulkan GUI upload")?;
        }

        let swapchains = [self.swapchain];
        let imageIndices = [imageIndex];
        let presentInfo = vk::PresentInfoKHR::default()
            .wait_semaphores(&signalSemaphores)
            .swapchains(&swapchains)
            .image_indices(&imageIndices);
        let presentSuboptimal = match unsafe {
            self.swapchainLoader.queue_present(self.presentQueue, &presentInfo)
        } {
            Ok(suboptimal) => suboptimal,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
            Err(error) => return Err(anyhow!("failed presenting Vulkan frame: {error:?}")),
        };

        self.currentFrame = (self.currentFrame + 1) % self.synchronization.len();
        if acquireSuboptimal || presentSuboptimal {
            // SUBOPTIMAL is harmless in the Vulkan contract (frames still
            // present correctly). On Android, Adreno drivers can report it
            // every frame for non-IDENTITY surface transforms; recreating the
            // swapchain in response would stall the frame loop. Only
            // OUT_OF_DATE (real size changes) and Resized events rebuild.
            log::debug!(
                "swapchain suboptimal after frame (acquire={acquireSuboptimal}, present={presentSuboptimal}); tolerating"
            );
        }
        Ok(())
    }

    /// Draws the MCP GUI command stream directly with Vulkan. The CPU still
    /// prepares source-ordered Tessellator-equivalent vertices and resolves
    /// resource-pack textures, but no full-window CPU raster image is created
    /// or uploaded during the normal Vulkan menu path.
    pub fn drawNativeGuiFrame(
        &mut self,
        window: &Window,
        frame: &GuiRenderFrame,
    ) -> anyhow::Result<()> {
        if self.swapchain == vk::SwapchainKHR::null() {
            self.recreateSwapchain(window)?;
            if self.swapchain == vk::SwapchainKHR::null() {
                return Ok(());
            }
        }
        self.ensureGuiPipeline()?;
        let frameSlot = self.currentFrame;
        let (imageAvailableSemaphore, renderFinishedSemaphore, inFlightFence) = {
            let synchronization = &self.synchronization[frameSlot];
            (
                synchronization.imageAvailableSemaphore,
                synchronization.renderFinishedSemaphore,
                synchronization.inFlightFence,
            )
        };
        unsafe {
            self.device
                .wait_for_fences(&[inFlightFence], true, u64::MAX)
                .context("failed waiting for Vulkan GUI frame fence")?;
        }

        let acquired = unsafe {
            self.swapchainLoader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                imageAvailableSemaphore,
                vk::Fence::null(),
            )
        };
        let (imageIndex, acquireSuboptimal) = match acquired {
            Ok(value) => value,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                log::debug!("swapchain OUT_OF_DATE on acquire; recreating");
                self.recreateSwapchain(window)?;
                return Ok(());
            }
            Err(error) => return Err(anyhow!("failed to acquire Vulkan GUI image: {error:?}")),
        };

        let imageFence = self.imagesInFlight[imageIndex as usize];
        if imageFence != vk::Fence::null() {
            unsafe {
                self.device
                    .wait_for_fences(&[imageFence], true, u64::MAX)
                    .context("failed waiting for Vulkan GUI swapchain-image fence")?;
            }
        }
        self.imagesInFlight[imageIndex as usize] = inFlightFence;

        self.guiPipeline
            .as_mut()
            .context("Vulkan GUI pipeline was not initialized")?
            .record(
                &self.device,
                &self.memoryProperties,
                self.commandPool,
                self.graphicsQueue,
                self.commandBuffers[imageIndex as usize],
                frameSlot,
                imageIndex as usize,
                self.swapchainExtent,
                frame,
            )?;
        self.imageInitialized[imageIndex as usize] = true;

        unsafe {
            self.device
                .reset_fences(&[inFlightFence])
                .context("failed resetting Vulkan GUI frame fence")?;
        }
        let waitSemaphores = [imageAvailableSemaphore];
        let waitStages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let commandBuffers = [self.commandBuffers[imageIndex as usize]];
        let signalSemaphores = [renderFinishedSemaphore];
        let submitInfo = vk::SubmitInfo::default()
            .wait_semaphores(&waitSemaphores)
            .wait_dst_stage_mask(&waitStages)
            .command_buffers(&commandBuffers)
            .signal_semaphores(&signalSemaphores);
        unsafe {
            self.device
                .queue_submit(self.graphicsQueue, &[submitInfo], inFlightFence)
                .context("failed submitting Vulkan native GUI draw")?;
        }

        let swapchains = [self.swapchain];
        let imageIndices = [imageIndex];
        let presentInfo = vk::PresentInfoKHR::default()
            .wait_semaphores(&signalSemaphores)
            .swapchains(&swapchains)
            .image_indices(&imageIndices);
        let presentSuboptimal = match unsafe {
            self.swapchainLoader.queue_present(self.presentQueue, &presentInfo)
        } {
            Ok(suboptimal) => suboptimal,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
            Err(error) => return Err(anyhow!("failed presenting Vulkan GUI frame: {error:?}")),
        };

        self.currentFrame = (self.currentFrame + 1) % self.synchronization.len();
        if acquireSuboptimal || presentSuboptimal {
            // SUBOPTIMAL is harmless in the Vulkan contract (frames still
            // present correctly). On Android, Adreno drivers can report it
            // every frame for non-IDENTITY surface transforms; recreating the
            // swapchain in response would stall the frame loop. Only
            // OUT_OF_DATE (real size changes) and Resized events rebuild.
            log::debug!(
                "swapchain suboptimal after frame (acquire={acquireSuboptimal}, present={presentSuboptimal}); tolerating"
            );
        }
        Ok(())
    }

    /// Draws a real GPU world frame. Unlike `drawFrame`, no finished pixel
    /// image is supplied by the CPU: the chunk vertex/index buffers are bound
    /// and Vulkan rasterizes the world directly into the swapchain image.
    pub fn drawWorldFrame(
        &mut self,
        window: &Window,
        frame: &WorldRenderFrame,
    ) -> anyhow::Result<()> {
        if self.swapchain == vk::SwapchainKHR::null() {
            self.recreateSwapchain(window)?;
            if self.swapchain == vk::SwapchainKHR::null() {
                return Ok(());
            }
        }
        self.ensureWorldPipeline()?;
        let frameSlot = self.currentFrame;

        let (imageAvailableSemaphore, renderFinishedSemaphore, inFlightFence) = {
            let synchronization = &self.synchronization[frameSlot];
            (
                synchronization.imageAvailableSemaphore,
                synchronization.renderFinishedSemaphore,
                synchronization.inFlightFence,
            )
        };
        unsafe {
            self.device
                .wait_for_fences(&[inFlightFence], true, u64::MAX)
                .context("failed waiting for Vulkan world frame fence")?;
        }

        let acquired = unsafe {
            self.swapchainLoader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                imageAvailableSemaphore,
                vk::Fence::null(),
            )
        };
        let (imageIndex, acquireSuboptimal) = match acquired {
            Ok(value) => value,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                log::debug!("swapchain OUT_OF_DATE on acquire; recreating");
                self.recreateSwapchain(window)?;
                return Ok(());
            }
            Err(error) => {
                return Err(anyhow!(
                    "failed to acquire world swapchain image: {error:?}"
                ));
            }
        };

        let imageFence = self.imagesInFlight[imageIndex as usize];
        if imageFence != vk::Fence::null() {
            unsafe {
                self.device
                    .wait_for_fences(&[imageFence], true, u64::MAX)
                    .context("failed waiting for world swapchain-image fence")?;
            }
        }
        self.imagesInFlight[imageIndex as usize] = inFlightFence;

        {
            let worldPipeline = self
                .worldPipeline
                .as_mut()
                .context("Vulkan world pipeline was not initialized")?;
            worldPipeline.collect_retired(&self.device, frameSlot)?;
            worldPipeline.upload_frame_mesh(
                &self.device,
                &self.memoryProperties,
                self.commandPool,
                self.graphicsQueue,
                frameSlot,
                frame,
            )?;
        }

        self.worldPipeline
            .as_mut()
            .context("Vulkan world pipeline was not initialized")?
            .record(
                &self.device,
                self.commandBuffers[imageIndex as usize],
                frameSlot,
                imageIndex as usize,
                self.swapchainExtent,
                frame,
            )?;
        self.imageInitialized[imageIndex as usize] = true;

        unsafe {
            self.device
                .reset_fences(&[inFlightFence])
                .context("failed resetting Vulkan world-frame fence")?;
        }
        let waitSemaphores = [imageAvailableSemaphore];
        let waitStages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let commandBuffers = [self.commandBuffers[imageIndex as usize]];
        let signalSemaphores = [renderFinishedSemaphore];
        let submitInfo = vk::SubmitInfo::default()
            .wait_semaphores(&waitSemaphores)
            .wait_dst_stage_mask(&waitStages)
            .command_buffers(&commandBuffers)
            .signal_semaphores(&signalSemaphores);
        unsafe {
            self.device
                .queue_submit(self.graphicsQueue, &[submitInfo], inFlightFence)
                .context("failed submitting Vulkan world draw")?;
        }

        let swapchains = [self.swapchain];
        let imageIndices = [imageIndex];
        let presentInfo = vk::PresentInfoKHR::default()
            .wait_semaphores(&signalSemaphores)
            .swapchains(&swapchains)
            .image_indices(&imageIndices);
        let presentSuboptimal = match unsafe {
            self.swapchainLoader
                .queue_present(self.presentQueue, &presentInfo)
        } {
            Ok(suboptimal) => suboptimal,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
            Err(error) => {
                return Err(anyhow!(
                    "failed presenting Vulkan world frame: {error:?}"
                ));
            }
        };

        self.currentFrame = (self.currentFrame + 1) % self.synchronization.len();
        if acquireSuboptimal || presentSuboptimal {
            // SUBOPTIMAL is harmless in the Vulkan contract (frames still
            // present correctly). On Android, Adreno drivers can report it
            // every frame for non-IDENTITY surface transforms; recreating the
            // swapchain in response would stall the frame loop. Only
            // OUT_OF_DATE (real size changes) and Resized events rebuild.
            log::debug!(
                "swapchain suboptimal after frame (acquire={acquireSuboptimal}, present={presentSuboptimal}); tolerating"
            );
        }
        Ok(())
    }

    pub fn resize(&mut self, window: &Window) -> anyhow::Result<()> {
        self.recreateSwapchain(window)
    }

    /// Runtime equivalent of `Display.setVSyncEnabled`. Vulkan present mode is
    /// part of the swapchain, so changing this option recreates only the
    /// swapchain-dependent resources and preserves resident chunk buffers.
    pub fn setVsync(&mut self, window: &Window, enableVsync: bool) -> anyhow::Result<()> {
        if self.enableVsync == enableVsync {
            return Ok(());
        }
        self.enableVsync = enableVsync;
        self.recreateSwapchain(window)
    }

    pub const fn isVsyncEnabled(&self) -> bool { self.enableVsync }

    fn createSwapchainObjects(&mut self, window: &Window) -> anyhow::Result<()> {
        let (width, height) = self.nativeSurfaceSize(window);
        if width == 0 || height == 0 {
            return Ok(());
        }

        let capabilities = unsafe {
            self.surfaceLoader.get_physical_device_surface_capabilities(
                self.physicalDevice,
                self.surface,
            )
        }
        .context("failed querying Vulkan surface capabilities")?;
        log::debug!(
            "surface capabilities: current_extent={:?}, min={:?}, max={:?}, current_transform={:?}, supported_transforms={:?}, composite_alpha={:?}",
            capabilities.current_extent,
            capabilities.min_image_extent,
            capabilities.max_image_extent,
            capabilities.current_transform,
            capabilities.supported_transforms,
            capabilities.supported_composite_alpha,
        );
        let requiredSwapchainUsage =
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST;
        anyhow::ensure!(
            capabilities
                .supported_usage_flags
                .contains(requiredSwapchainUsage),
            "Vulkan surface does not support both color-attachment and transfer-destination swapchain usage"
        );
        let formats = unsafe {
            self.surfaceLoader.get_physical_device_surface_formats(
                self.physicalDevice,
                self.surface,
            )
        }
        .context("failed querying Vulkan surface formats")?;
        let presentModes = unsafe {
            self.surfaceLoader.get_physical_device_surface_present_modes(
                self.physicalDevice,
                self.surface,
            )
        }
        .context("failed querying Vulkan present modes")?;
        let choice = choose_swapchain(
            &capabilities,
            &formats,
            &presentModes,
            width,
            height,
            self.enableVsync,
        )
        .context("surface did not expose a usable swapchain format")?;
        // Android: current_transform can report ROTATE_90 while the content
        // is landscape (winit creates the surface before the activity
        // rotation settles). Using ROTATE_90 would flip the image sideways;
        // keep the renderer's orientation by preferring IDENTITY and tolerating
        // the resulting SUBOPTIMAL present (frames still display correctly).
        #[cfg(target_os = "android")]
        let pre_transform = if capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            capabilities.current_transform
        };
        #[cfg(not(target_os = "android"))]
        let pre_transform = choice.pre_transform;
        log::info!(
            "Vulkan swapchain: {}x{}, format={:?}, present={:?}, vsync={}",
            choice.extent.width,
            choice.extent.height,
            choice.surface_format.format,
            choice.present_mode,
            self.enableVsync,
        );
        anyhow::ensure!(
            matches!(
                choice.surface_format.format,
                vk::Format::B8G8R8A8_UNORM
                    | vk::Format::B8G8R8A8_SRGB
                    | vk::Format::R8G8B8A8_UNORM
                    | vk::Format::R8G8B8A8_SRGB
            ),
            "Vulkan surface selected unsupported GUI upload format {:?}",
            choice.surface_format.format,
        );

        let queueIndices = [self.queueFamilies.graphics, self.queueFamilies.present];
        let mut createInfo = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(choice.image_count)
            .image_format(choice.surface_format.format)
            .image_color_space(choice.surface_format.color_space)
            .image_extent(choice.extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .pre_transform(pre_transform)
            .composite_alpha(choice.composite_alpha)
            .present_mode(choice.present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());
        if self.queueFamilies.graphics != self.queueFamilies.present {
            createInfo = createInfo
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&queueIndices);
        } else {
            createInfo = createInfo.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }

        self.swapchain = unsafe { self.swapchainLoader.create_swapchain(&createInfo, None) }
            .context("failed to create Vulkan swapchain")?;
        self.swapchainImages = unsafe { self.swapchainLoader.get_swapchain_images(self.swapchain) }
            .context("failed to obtain Vulkan swapchain images")?;
        self.swapchainFormat = choice.surface_format.format;
        self.swapchainExtent = choice.extent;

        let allocationInfo = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.commandPool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(self.swapchainImages.len() as u32);
        self.commandBuffers = unsafe { self.device.allocate_command_buffers(&allocationInfo) }
            .context("failed to allocate Vulkan command buffers")?;
        self.uploadBuffers = (0..self.swapchainImages.len())
            .map(|_| self.createUploadBuffer())
            .collect::<anyhow::Result<Vec<_>>>()?;
        self.imageInitialized = vec![false; self.swapchainImages.len()];
        self.imagesInFlight = vec![vk::Fence::null(); self.swapchainImages.len()];
        Ok(())
    }

    fn createUploadBuffer(&self) -> anyhow::Result<UploadBuffer> {
        let size = self.swapchainExtent.width as vk::DeviceSize
            * self.swapchainExtent.height as vk::DeviceSize
            * 4;
        let bufferInfo = vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { self.device.create_buffer(&bufferInfo, None) }
            .context("failed creating Vulkan GUI upload buffer")?;
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let memoryType = findMemoryType(
            &self.memoryProperties,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .context("no host-visible coherent Vulkan memory type for GUI upload")?;
        let allocationInfo = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memoryType);
        let memory = match unsafe { self.device.allocate_memory(&allocationInfo, None) } {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { self.device.destroy_buffer(buffer, None) };
                return Err(anyhow!("failed allocating Vulkan GUI upload memory: {error:?}"));
            }
        };
        if let Err(error) = unsafe { self.device.bind_buffer_memory(buffer, memory, 0) } {
            unsafe {
                self.device.free_memory(memory, None);
                self.device.destroy_buffer(buffer, None);
            }
            return Err(anyhow!("failed binding Vulkan GUI upload memory: {error:?}"));
        }
        let mapped = match unsafe { self.device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) } {
            Ok(pointer) => NonNull::new(pointer.cast::<u8>())
                .context("Vulkan returned a null mapped GUI upload pointer")?,
            Err(error) => {
                unsafe {
                    self.device.free_memory(memory, None);
                    self.device.destroy_buffer(buffer, None);
                }
                return Err(anyhow!("failed mapping Vulkan GUI upload memory: {error:?}"));
            }
        };
        Ok(UploadBuffer { buffer, memory, mapped, size })
    }

    fn uploadFrame(&self, imageIndex: usize, framePixels: &CpuFrame) -> anyhow::Result<()> {
        let upload = &self.uploadBuffers[imageIndex];
        let mapped = unsafe {
            std::slice::from_raw_parts_mut(upload.mapped.as_ptr(), upload.size as usize)
        };
        framePixels.write_for_vulkan_format(self.swapchainFormat, mapped)
    }

    fn recordUploadCommandBuffer(&mut self, imageIndex: usize) -> anyhow::Result<()> {
        let commandBuffer = self.commandBuffers[imageIndex];
        unsafe {
            self.device
                .reset_command_buffer(commandBuffer, vk::CommandBufferResetFlags::empty())
                .context("failed resetting Vulkan GUI command buffer")?;
        }
        let beginInfo = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .begin_command_buffer(commandBuffer, &beginInfo)
                .context("failed beginning Vulkan GUI command buffer")?;
        }

        let oldLayout = if self.imageInitialized[imageIndex] {
            vk::ImageLayout::PRESENT_SRC_KHR
        } else {
            vk::ImageLayout::UNDEFINED
        };
        let toTransfer = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(oldLayout)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.swapchainImages[imageIndex])
            .subresource_range(colorSubresourceRange());
        unsafe {
            self.device.cmd_pipeline_barrier(
                commandBuffer,
                if self.imageInitialized[imageIndex] {
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE
                } else {
                    vk::PipelineStageFlags::TOP_OF_PIPE
                },
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[toTransfer],
            );
        }

        let copyRegion = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width: self.swapchainExtent.width,
                height: self.swapchainExtent.height,
                depth: 1,
            });
        unsafe {
            self.device.cmd_copy_buffer_to_image(
                commandBuffer,
                self.uploadBuffers[imageIndex].buffer,
                self.swapchainImages[imageIndex],
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[copyRegion],
            );
        }

        let toPresent = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.swapchainImages[imageIndex])
            .subresource_range(colorSubresourceRange());
        unsafe {
            self.device.cmd_pipeline_barrier(
                commandBuffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[toPresent],
            );
            self.device
                .end_command_buffer(commandBuffer)
                .context("failed ending Vulkan GUI command buffer")?;
        }
        self.imageInitialized[imageIndex] = true;
        Ok(())
    }

    fn createSynchronizationObjects(&mut self) -> anyhow::Result<()> {
        let semaphoreInfo = vk::SemaphoreCreateInfo::default();
        let fenceInfo = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            let imageAvailableSemaphore = unsafe {
                self.device.create_semaphore(&semaphoreInfo, None)
            }
            .context("failed to create image-available semaphore")?;
            let renderFinishedSemaphore = unsafe {
                self.device.create_semaphore(&semaphoreInfo, None)
            }
            .context("failed to create render-finished semaphore")?;
            let inFlightFence = unsafe { self.device.create_fence(&fenceInfo, None) }
                .context("failed to create in-flight fence")?;
            self.synchronization.push(FrameSynchronization {
                imageAvailableSemaphore,
                renderFinishedSemaphore,
                inFlightFence,
            });
        }
        Ok(())
    }

    fn ensureGuiPipeline(&mut self) -> anyhow::Result<()> {
        if self.guiPipeline.is_some() || self.swapchain == vk::SwapchainKHR::null() {
            return Ok(());
        }
        self.guiPipeline = Some(VulkanGuiPipeline::new(
            &self.device,
            &self.memoryProperties,
            &self.swapchainImages,
            self.swapchainFormat,
            self.swapchainExtent,
            MAX_FRAMES_IN_FLIGHT,
        )?);
        Ok(())
    }

    fn ensureWorldPipeline(&mut self) -> anyhow::Result<()> {
        if self.worldPipeline.is_some() || self.swapchain == vk::SwapchainKHR::null() {
            return Ok(());
        }
        self.worldPipeline = Some(WorldGpuPipeline::new(
            &self.instance,
            self.physicalDevice,
            &self.device,
            &self.memoryProperties,
            &self.swapchainImages,
            self.swapchainFormat,
            self.swapchainExtent,
            MAX_FRAMES_IN_FLIGHT,
            self.wideLinesEnabled,
            self.multiDrawIndirectEnabled,
        )?);
        Ok(())
    }

    /// Android: winit's reported inner size can differ from the actual
    /// ANativeWindow dimensions (system bars, cutouts), which makes every
    /// present return VK_SUBOPTIMAL_KHR. Read the native window instead.
    fn nativeSurfaceSize(&self, window: &Window) -> (u32, u32) {
        #[cfg(target_os = "android")]
        {
            use raw_window_handle::HasWindowHandle;
            if let Ok(handle) = window.window_handle() {
                if let raw_window_handle::RawWindowHandle::AndroidNdk(android) = handle.as_raw() {
                    let native = android.a_native_window.as_ptr() as *mut ndk_sys::ANativeWindow;
                    let w = unsafe { ndk_sys::ANativeWindow_getWidth(native) };
                    let h = unsafe { ndk_sys::ANativeWindow_getHeight(native) };
                    if w > 0 && h > 0 {
                        log::debug!("native surface size: {w}x{h}");
                        return (w as u32, h as u32);
                    }
                    log::debug!("native window reported invalid size {w}x{h}; falling back to winit");
                }
            }
        }
        let size = window.inner_size();
        log::debug!("using winit inner size: {}x{}", size.width, size.height);
        (size.width, size.height)
    }

    fn recreateSwapchain(&mut self, window: &Window) -> anyhow::Result<()> {
        let (width, height) = self.nativeSurfaceSize(window);
        if width == 0 || height == 0 {
            return Ok(());
        }
        unsafe {
            self.device
                .device_wait_idle()
                .context("failed waiting for Vulkan device before swapchain recreation")?;
        }
        if let Some(worldPipeline) = self.worldPipeline.as_mut() {
            worldPipeline.destroy_swapchain_resources(&self.device);
        }
        if let Some(guiPipeline) = self.guiPipeline.as_mut() {
            guiPipeline.destroy_swapchain_resources(&self.device);
        }
        self.destroySwapchainObjects();
        self.createSwapchainObjects(window)?;
        if let Some(worldPipeline) = self.worldPipeline.as_mut() {
            worldPipeline.recreate_swapchain_resources(
                &self.device,
                &self.memoryProperties,
                &self.swapchainImages,
                self.swapchainFormat,
                self.swapchainExtent,
            )?;
        } else {
            self.ensureWorldPipeline()?;
        }
        if let Some(guiPipeline) = self.guiPipeline.as_mut() {
            guiPipeline.recreate_swapchain_resources(
                &self.device,
                &self.swapchainImages,
                self.swapchainFormat,
                self.swapchainExtent,
            )?;
        } else {
            self.ensureGuiPipeline()?;
        }
        Ok(())
    }

    fn destroySwapchainObjects(&mut self) {
        unsafe {
            if !self.commandBuffers.is_empty() {
                self.device.free_command_buffers(self.commandPool, &self.commandBuffers);
            }
            self.commandBuffers.clear();
            for upload in self.uploadBuffers.drain(..) {
                self.device.unmap_memory(upload.memory);
                self.device.destroy_buffer(upload.buffer, None);
                self.device.free_memory(upload.memory, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchainLoader.destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
        self.swapchainImages.clear();
        self.imageInitialized.clear();
        self.imagesInFlight.clear();
        self.swapchainExtent = vk::Extent2D { width: 0, height: 0 };
    }
}

impl Drop for VulkanWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            for synchronization in self.synchronization.drain(..) {
                self.device.destroy_semaphore(synchronization.imageAvailableSemaphore, None);
                self.device.destroy_semaphore(synchronization.renderFinishedSemaphore, None);
                self.device.destroy_fence(synchronization.inFlightFence, None);
            }
            if let Some(mut worldPipeline) = self.worldPipeline.take() {
                worldPipeline.destroy(&self.device);
            }
            if let Some(mut guiPipeline) = self.guiPipeline.take() {
                guiPipeline.destroy(&self.device);
            }
            self.destroySwapchainObjects();
            self.device.destroy_command_pool(self.commandPool, None);
            self.device.destroy_device(None);
            self.surfaceLoader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
        let _ = &self.entry;
    }
}

fn colorSubresourceRange() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
}

fn findMemoryType(
    properties: &vk::PhysicalDeviceMemoryProperties,
    allowedTypes: u32,
    required: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..properties.memory_type_count).find(|&index| {
        allowedTypes & (1 << index) != 0
            && properties.memory_types[index as usize]
                .property_flags
                .contains(required)
    })
}

unsafe fn selectPhysicalDevice(
    instance: &Instance,
    surfaceLoader: &khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> anyhow::Result<(vk::PhysicalDevice, QueueFamilies)> {
    let devices = instance
        .enumerate_physical_devices()
        .context("failed enumerating Vulkan physical devices")?;
    let mut candidates = Vec::new();
    for device in devices {
        let properties = instance.get_physical_device_properties(device);
        let queueProperties = instance.get_physical_device_queue_family_properties(device);
        let graphics = queueProperties
            .iter()
            .position(|queue| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|value| value as u32);
        let mut present = None;
        for index in 0..queueProperties.len() {
            if surfaceLoader
                .get_physical_device_surface_support(device, index as u32, surface)
                .unwrap_or(false)
            {
                present = Some(index as u32);
                break;
            }
        }
        if let (Some(graphics), Some(present)) = (graphics, present) {
            let rank = match properties.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                vk::PhysicalDeviceType::CPU => 3,
                _ => 4,
            };
            candidates.push((rank, device, QueueFamilies { graphics, present }));
        }
    }
    candidates.sort_by_key(|candidate| candidate.0);
    candidates
        .into_iter()
        .next()
        .map(|(_, device, queues)| (device, queues))
        .ok_or_else(|| anyhow!("no physical device can present to the created surface"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_type_filter_requires_all_requested_flags() {
        let mut properties = vk::PhysicalDeviceMemoryProperties::default();
        properties.memory_type_count = 2;
        properties.memory_types[0].property_flags = vk::MemoryPropertyFlags::HOST_VISIBLE;
        properties.memory_types[1].property_flags =
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        assert_eq!(
            findMemoryType(
                &properties,
                0b11,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            ),
            Some(1),
        );
    }
}
