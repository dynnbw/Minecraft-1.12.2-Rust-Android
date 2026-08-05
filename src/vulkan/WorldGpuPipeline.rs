use std::collections::HashMap;
use std::ffi::CString;
use std::io::Cursor;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use ash::{vk, Device, Instance};

use crate::net::minecraft::util::BlockRenderLayer::BlockRenderLayer;
use crate::net::minecraft::client::renderer::chunk::RenderChunk::RenderChunkKey;
use crate::vulkan::VulkanWorldRenderer::{
    ChunkLayerRange, EntityOverlayPipelineKind, FirstPersonPipelineKind, HudPipelineKind,
    WorldEntityMeshKind, WorldRenderFrame, WorldVertex,
};

// The terrain submission path keeps immutable RenderChunk geometry in bounded
// shared device-local arenas.  Minecraft still owns each RenderChunk revision,
// visibility decision and layer range; the backend only replaces repeated
// buffer binds with indexed-indirect command streams.  Arena exhaustion falls
// back to dedicated buffers, so geometry is never discarded.
const CHUNK_VERTEX_ARENA_BYTES: vk::DeviceSize = 96 * 1024 * 1024;
const CHUNK_INDEX_ARENA_BYTES: vk::DeviceSize = 48 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FreeSpan {
    start: u32,
    count: u32,
}

#[derive(Debug)]
struct ElementArenaAllocator {
    /// Sorted, non-overlapping free spans.  `recycle` inserts directly at the
    /// correct position and merges neighbours, avoiding a full sort on every
    /// completed frame-slot retirement.
    freeSpans: Vec<FreeSpan>,
}

impl ElementArenaAllocator {
    fn new(capacity: u32) -> Self {
        Self {
            freeSpans: vec![FreeSpan {
                start: 0,
                count: capacity,
            }],
        }
    }

    fn claim(&mut self, count: u32) -> Option<u32> {
        if count == 0 {
            return Some(0);
        }

        // Best-fit reduces fragmentation when differently sized 16x16x16
        // RenderChunk meshes are replaced over long sessions.
        let (index, _) = self
            .freeSpans
            .iter()
            .enumerate()
            .filter(|(_, span)| span.count >= count)
            .min_by_key(|(_, span)| span.count - count)?;
        let start = self.freeSpans[index].start;
        if self.freeSpans[index].count == count {
            self.freeSpans.remove(index);
        } else {
            self.freeSpans[index].start = self.freeSpans[index].start.saturating_add(count);
            self.freeSpans[index].count -= count;
        }
        Some(start)
    }

    fn recycle(&mut self, start: u32, count: u32) {
        if count == 0 {
            return;
        }
        let end = start.saturating_add(count);
        let insertAt = self.freeSpans.partition_point(|span| span.start < start);
        let mergeLeft = insertAt > 0
            && self.freeSpans[insertAt - 1]
                .start
                .saturating_add(self.freeSpans[insertAt - 1].count)
                == start;
        let mergeRight = insertAt < self.freeSpans.len()
            && end == self.freeSpans[insertAt].start;

        match (mergeLeft, mergeRight) {
            (true, true) => {
                let rightCount = self.freeSpans[insertAt].count;
                self.freeSpans[insertAt - 1].count = self.freeSpans[insertAt - 1]
                    .count
                    .saturating_add(count)
                    .saturating_add(rightCount);
                self.freeSpans.remove(insertAt);
            }
            (true, false) => {
                self.freeSpans[insertAt - 1].count = self.freeSpans[insertAt - 1]
                    .count
                    .saturating_add(count);
            }
            (false, true) => {
                self.freeSpans[insertAt].start = start;
                self.freeSpans[insertAt].count = self.freeSpans[insertAt]
                    .count
                    .saturating_add(count);
            }
            (false, false) => self.freeSpans.insert(insertAt, FreeSpan { start, count }),
        }
    }

    fn free_count(&self) -> u64 {
        self.freeSpans
            .iter()
            .map(|span| span.count as u64)
            .sum()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct ChunkIndirectCommand {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    vertex_offset: i32,
    first_instance: u32,
}

struct GpuBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
}

struct GpuTexture {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    sampler: vk::Sampler,
}

struct PendingTextureUpload {
    staging: GpuBuffer,
    image: vk::Image,
    width: u32,
    height: u32,
}

struct FrameStagingBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped: NonNull<u8>,
    capacity: vk::DeviceSize,
}

#[derive(Clone, Copy)]
struct PendingBufferCopy {
    destination: vk::Buffer,
    region: vk::BufferCopy,
    destinationAccess: vk::AccessFlags,
}

enum ChunkStorage {
    Shared {
        firstVertex: u32,
        vertexCount: u32,
        firstIndex: u32,
        indexCount: u32,
    },
    Dedicated {
        vertexBuffer: GpuBuffer,
        indexBuffer: GpuBuffer,
    },
}

struct GpuChunk {
    storage: ChunkStorage,
    layerRanges: [ChunkLayerRange; 4],
    vertexCount: u32,
    indexCount: u32,
    meshRevision: u64,
}

struct GpuEntityMesh {
    vertexBuffer: GpuBuffer,
    indexBuffer: GpuBuffer,
    indexCount: u32,
    /// reference-renderer-style monotonic CPU mesh generation. Every frame slot retains
    /// its device-local buffers and skips both hashing and staging when it has
    /// already uploaded the current generation.
    contentGeneration: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineBlendMode {
    Disabled,
    Alpha,
    InvertCrosshair,
    Glint,
    BlockDamage,
    /// RenderTntMinecart white overlay: blendFunc(SRC_ALPHA, DST_ALPHA).
    TntFlash,
    /// TileEntityEndPortalRenderer layers j >= 1: blendFunc(ONE, ONE).
    Additive,
    /// RenderGlobal celestial pass: blendFunc(SRC_ALPHA, ONE).
    SourceAlphaAdditive,
}

/// Owns the native Vulkan resources corresponding to MCP 1.12.2's
/// `RenderChunk` display lists / VBOs. Chunk meshes retain independent MCP
/// lifetime/revision ownership while their immutable geometry is suballocated
/// from shared device-local arenas for batched submission. Replacing one chunk
/// never rebuilds unrelated chunks or changes `RenderGlobal` visibility order.
pub struct WorldGpuPipeline {
    descriptorSetLayout: vk::DescriptorSetLayout,
    descriptorPool: vk::DescriptorPool,
    descriptorSets: Vec<vk::DescriptorSet>,
    pipelineLayout: vk::PipelineLayout,
    renderPass: vk::RenderPass,
    opaquePipeline: vk::Pipeline,
    entityPipeline: vk::Pipeline,
    /// `RenderTntMinecart` texture-disabled white fuse flash.
    entityOverlayPipeline: vk::Pipeline,
    endPortalAdditivePipeline: vk::Pipeline,
    beaconCorePipeline: vk::Pipeline,
    beaconGlowPipeline: vk::Pipeline,
    skyCelestialPipeline: vk::Pipeline,
    /// `ModelBoat#renderMultipass`: entity alpha-test/depth semantics with
    /// `GlStateManager.colorMask(false, false, false, false)`.
    entityDepthPipeline: vk::Pipeline,
    damagePipeline: vk::Pipeline,
    selectionPipeline: vk::Pipeline,
    firstPersonPipeline: vk::Pipeline,
    firstPersonFirePipeline: vk::Pipeline,
    firstPersonGlintPipeline: vk::Pipeline,
    translucentPipeline: vk::Pipeline,
    hudPipeline: vk::Pipeline,
    crosshairPipeline: vk::Pipeline,
    glintPipeline: vk::Pipeline,
    swapchainImageViews: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    depthImages: Vec<vk::Image>,
    depthMemories: Vec<vk::DeviceMemory>,
    depthViews: Vec<vk::ImageView>,
    depthFormat: vk::Format,
    chunks: HashMap<RenderChunkKey, GpuChunk>,
    chunkVertexArena: GpuBuffer,
    chunkIndexArena: GpuBuffer,
    chunkVertexRanges: ElementArenaAllocator,
    chunkIndexRanges: ElementArenaAllocator,
    chunkIndirectBuffers: Vec<Option<FrameStagingBuffer>>,
    chunkIndirectCommands: Vec<ChunkIndirectCommand>,
    chunkIndirectLayerOffsets: Vec<[vk::DeviceSize; 4]>,
    chunkIndirectLayerCounts: Vec<[u32; 4]>,
    /// Per frame-slot generation of the exact visible shared-chunk command
    /// stream. Unchanged visibility/storage reuses the mapped indirect buffer.
    chunkIndirectSignatures: Vec<Option<u64>>,
    retiredChunkStorage: Vec<Vec<ChunkStorage>>,
    multiDrawIndirect: bool,
    entityMeshes: Vec<Option<GpuEntityMesh>>,
    blockEntityMeshes: Vec<Option<GpuEntityMesh>>,
    staticEntityMeshes: Vec<Option<GpuEntityMesh>>,
    entityOverlayMeshes: Vec<Option<GpuEntityMesh>>,
    entityDepthMeshes: Vec<Option<GpuEntityMesh>>,
    particleMeshes: Vec<Option<GpuEntityMesh>>,
    transparentParticleMeshes: Vec<Option<GpuEntityMesh>>,
    damageMeshes: Vec<Option<GpuEntityMesh>>,
    selectionMeshes: Vec<Option<GpuEntityMesh>>,
    firstPersonMeshes: Vec<Option<GpuEntityMesh>>,
    hudMeshes: Vec<Option<GpuEntityMesh>>,
    retiredBuffers: Vec<Vec<GpuBuffer>>,
    stagingBuffers: Vec<Option<FrameStagingBuffer>>,
    pendingCopies: Vec<Vec<PendingBufferCopy>>,
    textures: Vec<Option<GpuTexture>>,
    pendingTextureUploads: Vec<Option<PendingTextureUpload>>,
    uploadedAtlasRevisions: Vec<u64>,
    loggedFirstChunkUpload: bool,
    loggedFirstDraw: bool,
    performanceLogStarted: Instant,
    performanceFrames: u64,
    selectionLineWidth: f32,
}

impl WorldGpuPipeline {
    pub fn new(
        instance: &Instance,
        physicalDevice: vk::PhysicalDevice,
        device: &Device,
        memoryProperties: &vk::PhysicalDeviceMemoryProperties,
        swapchainImages: &[vk::Image],
        swapchainFormat: vk::Format,
        swapchainExtent: vk::Extent2D,
        framesInFlight: usize,
        wideLinesEnabled: bool,
        multiDrawIndirect: bool,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(framesInFlight > 0, "world pipeline requires at least one frame slot");
        let descriptorBinding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let descriptorBindings = [descriptorBinding];
        let descriptorSetLayoutInfo =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&descriptorBindings);
        let descriptorSetLayout = unsafe {
            device.create_descriptor_set_layout(&descriptorSetLayoutInfo, None)
        }
        .context("failed creating world descriptor-set layout")?;

        let poolSize = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(framesInFlight as u32);
        let poolSizes = [poolSize];
        let descriptorPoolInfo = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&poolSizes)
            .max_sets(framesInFlight as u32);
        let descriptorPool = match unsafe { device.create_descriptor_pool(&descriptorPoolInfo, None) } {
            Ok(pool) => pool,
            Err(error) => {
                unsafe { device.destroy_descriptor_set_layout(descriptorSetLayout, None) };
                return Err(anyhow!("failed creating world descriptor pool: {error:?}"));
            }
        };
        let setLayouts = vec![descriptorSetLayout; framesInFlight];
        let descriptorAllocateInfo = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptorPool)
            .set_layouts(&setLayouts);
        let descriptorSets = match unsafe { device.allocate_descriptor_sets(&descriptorAllocateInfo) } {
            Ok(sets) => sets,
            Err(error) => {
                unsafe {
                    device.destroy_descriptor_pool(descriptorPool, None);
                    device.destroy_descriptor_set_layout(descriptorSetLayout, None);
                }
                return Err(anyhow!("failed allocating world descriptor set: {error:?}"));
            }
        };

        let pushRange = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<
                crate::vulkan::VulkanWorldRenderer::WorldPushConstants,
            >() as u32);
        let pushRanges = [pushRange];
        let pipelineLayouts = [descriptorSetLayout];
        let pipelineLayoutInfo = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&pipelineLayouts)
            .push_constant_ranges(&pushRanges);
        let pipelineLayout = match unsafe { device.create_pipeline_layout(&pipelineLayoutInfo, None) } {
            Ok(layout) => layout,
            Err(error) => {
                unsafe {
                    device.destroy_descriptor_pool(descriptorPool, None);
                    device.destroy_descriptor_set_layout(descriptorSetLayout, None);
                }
                return Err(anyhow!("failed creating world pipeline layout: {error:?}"));
            }
        };

        let depthFormat = find_depth_format(instance, physicalDevice)
            .context("Vulkan device has no supported depth attachment format")?;
        let chunkVertexArena = match create_device_local_buffer(
            device,
            memoryProperties,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            CHUNK_VERTEX_ARENA_BYTES,
        ) {
            Ok(buffer) => buffer,
            Err(error) => {
                unsafe {
                    device.destroy_pipeline_layout(pipelineLayout, None);
                    device.destroy_descriptor_pool(descriptorPool, None);
                    device.destroy_descriptor_set_layout(descriptorSetLayout, None);
                }
                return Err(error.context("failed creating shared chunk vertex arena"));
            }
        };
        let chunkIndexArena = match create_device_local_buffer(
            device,
            memoryProperties,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            CHUNK_INDEX_ARENA_BYTES,
        ) {
            Ok(buffer) => buffer,
            Err(error) => {
                destroy_buffer(device, Some(chunkVertexArena));
                unsafe {
                    device.destroy_pipeline_layout(pipelineLayout, None);
                    device.destroy_descriptor_pool(descriptorPool, None);
                    device.destroy_descriptor_set_layout(descriptorSetLayout, None);
                }
                return Err(error.context("failed creating shared chunk index arena"));
            }
        };
        let chunkVertexCapacity = u32::try_from(
            CHUNK_VERTEX_ARENA_BYTES / std::mem::size_of::<WorldVertex>() as u64,
        )
        .context("shared chunk vertex arena exceeds u32 element addressing")?;
        let chunkIndexCapacity = u32::try_from(
            CHUNK_INDEX_ARENA_BYTES / std::mem::size_of::<u32>() as u64,
        )
        .context("shared chunk index arena exceeds u32 element addressing")?;
        let mut result = Self {
            descriptorSetLayout,
            descriptorPool,
            descriptorSets,
            pipelineLayout,
            renderPass: vk::RenderPass::null(),
            opaquePipeline: vk::Pipeline::null(),
            entityPipeline: vk::Pipeline::null(),
            entityOverlayPipeline: vk::Pipeline::null(),
            endPortalAdditivePipeline: vk::Pipeline::null(),
            beaconCorePipeline: vk::Pipeline::null(),
            beaconGlowPipeline: vk::Pipeline::null(),
            skyCelestialPipeline: vk::Pipeline::null(),
            entityDepthPipeline: vk::Pipeline::null(),
            damagePipeline: vk::Pipeline::null(),
            selectionPipeline: vk::Pipeline::null(),
            firstPersonPipeline: vk::Pipeline::null(),
            firstPersonFirePipeline: vk::Pipeline::null(),
            firstPersonGlintPipeline: vk::Pipeline::null(),
            translucentPipeline: vk::Pipeline::null(),
            hudPipeline: vk::Pipeline::null(),
            crosshairPipeline: vk::Pipeline::null(),
            glintPipeline: vk::Pipeline::null(),
            swapchainImageViews: Vec::new(),
            framebuffers: Vec::new(),
            depthImages: Vec::new(),
            depthMemories: Vec::new(),
            depthViews: Vec::new(),
            depthFormat,
            chunks: HashMap::new(),
            chunkVertexArena,
            chunkIndexArena,
            chunkVertexRanges: ElementArenaAllocator::new(chunkVertexCapacity),
            chunkIndexRanges: ElementArenaAllocator::new(chunkIndexCapacity),
            chunkIndirectBuffers: (0..framesInFlight).map(|_| None).collect(),
            chunkIndirectCommands: Vec::new(),
            chunkIndirectLayerOffsets: vec![[0; 4]; framesInFlight],
            chunkIndirectLayerCounts: vec![[0; 4]; framesInFlight],
            chunkIndirectSignatures: vec![None; framesInFlight],
            retiredChunkStorage: (0..framesInFlight).map(|_| Vec::new()).collect(),
            multiDrawIndirect,
            entityMeshes: (0..framesInFlight).map(|_| None).collect(),
            blockEntityMeshes: (0..framesInFlight).map(|_| None).collect(),
            staticEntityMeshes: (0..framesInFlight).map(|_| None).collect(),
            entityOverlayMeshes: (0..framesInFlight).map(|_| None).collect(),
            entityDepthMeshes: (0..framesInFlight).map(|_| None).collect(),
            particleMeshes: (0..framesInFlight).map(|_| None).collect(),
            transparentParticleMeshes: (0..framesInFlight).map(|_| None).collect(),
            damageMeshes: (0..framesInFlight).map(|_| None).collect(),
            selectionMeshes: (0..framesInFlight).map(|_| None).collect(),
            firstPersonMeshes: (0..framesInFlight).map(|_| None).collect(),
            hudMeshes: (0..framesInFlight).map(|_| None).collect(),
            retiredBuffers: (0..framesInFlight).map(|_| Vec::new()).collect(),
            stagingBuffers: (0..framesInFlight).map(|_| None).collect(),
            pendingCopies: (0..framesInFlight).map(|_| Vec::new()).collect(),
            textures: (0..framesInFlight).map(|_| None).collect(),
            pendingTextureUploads: (0..framesInFlight).map(|_| None).collect(),
            uploadedAtlasRevisions: vec![0; framesInFlight],
            loggedFirstChunkUpload: false,
            loggedFirstDraw: false,
            performanceLogStarted: Instant::now(),
            performanceFrames: 0,
            selectionLineWidth: if wideLinesEnabled { 2.0 } else { 1.0 },
        };
        log::info!(
            "Vulkan shared chunk arenas: vertex={} MiB, index={} MiB, multi_draw_indirect={}",
            CHUNK_VERTEX_ARENA_BYTES / (1024 * 1024),
            CHUNK_INDEX_ARENA_BYTES / (1024 * 1024),
            multiDrawIndirect,
        );
        if let Err(error) = result.create_swapchain_resources(
            device,
            memoryProperties,
            swapchainImages,
            swapchainFormat,
            swapchainExtent,
        ) {
            result.destroy(device);
            return Err(error);
        }
        Ok(result)
    }

    pub fn recreate_swapchain_resources(
        &mut self,
        device: &Device,
        memoryProperties: &vk::PhysicalDeviceMemoryProperties,
        swapchainImages: &[vk::Image],
        swapchainFormat: vk::Format,
        swapchainExtent: vk::Extent2D,
    ) -> anyhow::Result<()> {
        self.destroy_swapchain_resources(device);
        self.create_swapchain_resources(
            device,
            memoryProperties,
            swapchainImages,
            swapchainFormat,
            swapchainExtent,
        )
    }

    /// Releases resources retired on an earlier use of this frame slot. The
    /// caller invokes this only after the slot's fence has signalled. Keeping
    /// replacements alive for a complete frame-slot cycle avoids both
    /// `device_wait_idle` and use-after-free by an older command buffer.
    pub fn collect_retired(&mut self, device: &Device, frameSlot: usize) -> anyhow::Result<()> {
        anyhow::ensure!(
            frameSlot < self.retiredBuffers.len() && frameSlot < self.retiredChunkStorage.len(),
            "world frame-slot index out of range",
        );
        let retiredChunkStorage = std::mem::take(&mut self.retiredChunkStorage[frameSlot]);
        for storage in retiredChunkStorage {
            match storage {
                ChunkStorage::Shared {
                    firstVertex,
                    vertexCount,
                    firstIndex,
                    indexCount,
                } => {
                    self.chunkVertexRanges.recycle(firstVertex, vertexCount);
                    self.chunkIndexRanges.recycle(firstIndex, indexCount);
                }
                ChunkStorage::Dedicated { vertexBuffer, indexBuffer } => {
                    destroy_buffer(device, Some(vertexBuffer));
                    destroy_buffer(device, Some(indexBuffer));
                }
            }
        }
        let retired = &mut self.retiredBuffers[frameSlot];
        for buffer in retired.drain(..) {
            destroy_buffer(device, Some(buffer));
        }
        Ok(())
    }

    pub fn upload_frame_mesh(
        &mut self,
        device: &Device,
        memoryProperties: &vk::PhysicalDeviceMemoryProperties,
        _commandPool: vk::CommandPool,
        _graphicsQueue: vk::Queue,
        frameSlot: usize,
        frame: &WorldRenderFrame,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            frameSlot < self.retiredBuffers.len(),
            "world frame-slot index out of range"
        );
        anyhow::ensure!(
            frameSlot < self.stagingBuffers.len() && frameSlot < self.pendingCopies.len(),
            "world staging frame-slot index out of range"
        );

        anyhow::ensure!(
            frameSlot < self.descriptorSets.len()
                && frameSlot < self.textures.len()
                && frameSlot < self.pendingTextureUploads.len()
                && frameSlot < self.uploadedAtlasRevisions.len(),
            "world texture frame-slot index out of range",
        );

        // Each frame slot owns its descriptor and image. The caller has already
        // waited for this slot's fence, so its previous texture can be replaced
        // without stalling either the device or the graphics queue.
        if self.uploadedAtlasRevisions[frameSlot] != frame.atlasRevision {
            destroy_texture(device, self.textures[frameSlot].take());
            if let Some(pending) = self.pendingTextureUploads[frameSlot].take() {
                destroy_buffer(device, Some(pending.staging));
            }
            let (texture, pending) = create_pending_texture_upload(
                device,
                memoryProperties,
                frame.atlas.width,
                frame.atlas.height,
                &frame.atlas.rgba,
            )?;
            let imageInfo = vk::DescriptorImageInfo::default()
                .sampler(texture.sampler)
                .image_view(texture.view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let imageInfos = [imageInfo];
            let write = vk::WriteDescriptorSet::default()
                .dst_set(self.descriptorSets[frameSlot])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&imageInfos);
            unsafe { device.update_descriptor_sets(&[write], &[]) };
            self.textures[frameSlot] = Some(texture);
            self.pendingTextureUploads[frameSlot] = Some(pending);
            self.uploadedAtlasRevisions[frameSlot] = frame.atlasRevision;
        }

        for key in &frame.removedChunks {
            if let Some(chunk) = self.chunks.remove(key) {
                self.retire_chunk(frameSlot, chunk);
            }
        }

        // The frame-slot fence has already signalled before this method is
        // called. It is therefore safe to reuse this slot's mapped staging
        // buffer and replace it when the current upload burst is larger.
        self.pendingCopies[frameSlot].clear();
        let mut uploadBytes = frame
            .chunkUploads
            .iter()
            .filter(|upload| {
                !upload.vertices.is_empty()
                    && !upload.indices.is_empty()
                    && !self
                        .chunks
                        .get(&upload.key)
                        .is_some_and(|chunk| chunk.meshRevision == upload.meshRevision)
            })
            .fold(0_u64, |total, upload| {
                let vertexBytes = std::mem::size_of_val(upload.vertices.as_slice()) as u64;
                let indexBytes = std::mem::size_of_val(upload.indices.as_slice()) as u64;
                let indexOnly = upload.verticesUnchanged
                    && self.chunks.get(&upload.key).is_some_and(|chunk| {
                        chunk.vertexCount as usize == upload.vertices.len()
                            && chunk.indexCount as usize == upload.indices.len()
                    });
                if indexOnly {
                    align_up(total, 4).saturating_add(indexBytes)
                } else {
                    let afterVertices = align_up(total, 4).saturating_add(vertexBytes);
                    align_up(afterVertices, 4).saturating_add(indexBytes)
                }
            });
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.entityMeshes[frameSlot],
            frame.entityVertices.as_slice(),
            frame.entityIndices.as_slice(),
            frame.entityMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.blockEntityMeshes[frameSlot],
            frame.blockEntityVertices.as_slice(),
            frame.blockEntityIndices.as_slice(),
            frame.blockEntityMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.staticEntityMeshes[frameSlot],
            frame.staticEntityVertices.as_slice(),
            frame.staticEntityIndices.as_slice(),
            frame.staticEntityMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.entityOverlayMeshes[frameSlot],
            frame.entityOverlayVertices.as_slice(),
            frame.entityOverlayIndices.as_slice(),
            frame.entityOverlayMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.entityDepthMeshes[frameSlot],
            frame.entityDepthVertices.as_slice(),
            frame.entityDepthIndices.as_slice(),
            frame.entityDepthMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.particleMeshes[frameSlot],
            frame.particleVertices.as_slice(),
            frame.particleIndices.as_slice(),
            frame.particleMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.transparentParticleMeshes[frameSlot],
            frame.transparentParticleVertices.as_slice(),
            frame.transparentParticleIndices.as_slice(),
            frame.transparentParticleMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.damageMeshes[frameSlot],
            frame.damageVertices.as_slice(),
            frame.damageIndices.as_slice(),
            frame.damageMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.selectionMeshes[frameSlot],
            frame.selectionVertices.as_slice(),
            frame.selectionIndices.as_slice(),
            frame.selectionMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.firstPersonMeshes[frameSlot],
            frame.firstPersonVertices.as_slice(),
            frame.firstPersonIndices.as_slice(),
            frame.firstPersonMeshGeneration,
        );
        uploadBytes = append_dynamic_upload_bytes(
            uploadBytes,
            &self.hudMeshes[frameSlot],
            frame.hudVertices.as_slice(),
            frame.hudIndices.as_slice(),
            frame.hudMeshGeneration,
        );
        if uploadBytes > 0 {
            ensure_staging_capacity(
                device,
                memoryProperties,
                &mut self.stagingBuffers[frameSlot],
                uploadBytes,
            )?;
        }

        let hadDrawableChunks = !self.chunks.is_empty();
        let mut stagingOffset = 0_u64;
        for upload in &frame.chunkUploads {
            if self
                .chunks
                .get(&upload.key)
                .is_some_and(|chunk| chunk.meshRevision == upload.meshRevision)
            {
                continue;
            }

            if upload.vertices.is_empty() || upload.indices.is_empty() {
                if let Some(chunk) = self.chunks.remove(&upload.key) {
                    self.retire_chunk(frameSlot, chunk);
                }
                continue;
            }

            let vertexBytes = as_bytes(upload.vertices.as_slice());
            let indexBytes = as_bytes(upload.indices.as_slice());
            let vertexCount = u32::try_from(upload.vertices.len())
                .context("RenderChunk vertex count exceeds u32")?;
            let indexCount = u32::try_from(upload.indices.len())
                .context("RenderChunk index count exceeds u32")?;

            let indexOnlyDestination = if upload.verticesUnchanged {
                self.chunks.get(&upload.key).and_then(|chunk| {
                    if chunk.vertexCount != vertexCount || chunk.indexCount != indexCount {
                        return None;
                    }
                    Some(match &chunk.storage {
                        ChunkStorage::Shared { firstIndex, .. } => (
                            self.chunkIndexArena.buffer,
                            *firstIndex as u64 * std::mem::size_of::<u32>() as u64,
                        ),
                        ChunkStorage::Dedicated { indexBuffer, .. } => (indexBuffer.buffer, 0),
                    })
                })
            } else {
                None
            };
            if let Some((indexDestination, indexDestinationOffset)) = indexOnlyDestination {
                let staging = self.stagingBuffers[frameSlot]
                    .as_ref()
                    .context("world staging buffer missing for translucent index upload")?;
                let indexOffset = align_up(stagingOffset, 4);
                let endOffset = indexOffset + indexBytes.len() as u64;
                anyhow::ensure!(
                    endOffset <= staging.capacity,
                    "translucent index upload exceeded its allocated staging capacity"
                );
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        indexBytes.as_ptr(),
                        staging.mapped.as_ptr().add(indexOffset as usize),
                        indexBytes.len(),
                    );
                }
                self.pendingCopies[frameSlot].push(PendingBufferCopy {
                    destination: indexDestination,
                    region: vk::BufferCopy {
                        src_offset: indexOffset,
                        dst_offset: indexDestinationOffset,
                        size: indexBytes.len() as u64,
                    },
                    destinationAccess: vk::AccessFlags::INDEX_READ,
                });
                stagingOffset = endOffset;
                if let Some(chunk) = self.chunks.get_mut(&upload.key) {
                    chunk.layerRanges = upload.layerRanges;
                    chunk.meshRevision = upload.meshRevision;
                }
                continue;
            }

            let sharedRanges = self
                .chunkVertexRanges
                .claim(vertexCount)
                .and_then(|firstVertex| {
                    if let Some(firstIndex) = self.chunkIndexRanges.claim(indexCount) {
                        Some((firstVertex, firstIndex))
                    } else {
                        self.chunkVertexRanges.recycle(firstVertex, vertexCount);
                        None
                    }
                });

            let storage = if let Some((firstVertex, firstIndex)) = sharedRanges {
                ChunkStorage::Shared {
                    firstVertex,
                    vertexCount,
                    firstIndex,
                    indexCount,
                }
            } else {
                // Preserve correctness when a very large render distance or a
                // highly tessellated resource pack exhausts the shared arena.
                // A bounded dedicated-buffer fallback preserves every RenderChunk when
                // the shared arena is temporarily full; no geometry is dropped.
                let vertexBuffer = create_device_local_buffer(
                    device,
                    memoryProperties,
                    vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                    vertexBytes.len() as vk::DeviceSize,
                )?;
                let indexBuffer = match create_device_local_buffer(
                    device,
                    memoryProperties,
                    vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                    indexBytes.len() as vk::DeviceSize,
                ) {
                    Ok(buffer) => buffer,
                    Err(error) => {
                        destroy_buffer(device, Some(vertexBuffer));
                        return Err(error);
                    }
                };
                ChunkStorage::Dedicated { vertexBuffer, indexBuffer }
            };

            let staging = self.stagingBuffers[frameSlot]
                .as_ref()
                .context("world staging buffer missing for pending mesh upload")?;
            let vertexOffset = align_up(stagingOffset, 4);
            let indexOffset = align_up(vertexOffset + vertexBytes.len() as u64, 4);
            let endOffset = indexOffset + indexBytes.len() as u64;
            anyhow::ensure!(
                endOffset <= staging.capacity,
                "world staging upload exceeded its allocated capacity"
            );
            unsafe {
                std::ptr::copy_nonoverlapping(
                    vertexBytes.as_ptr(),
                    staging.mapped.as_ptr().add(vertexOffset as usize),
                    vertexBytes.len(),
                );
                std::ptr::copy_nonoverlapping(
                    indexBytes.as_ptr(),
                    staging.mapped.as_ptr().add(indexOffset as usize),
                    indexBytes.len(),
                );
            }
            let (vertexDestination, vertexDestinationOffset, indexDestination, indexDestinationOffset) =
                match &storage {
                    ChunkStorage::Shared { firstVertex, firstIndex, .. } => (
                        self.chunkVertexArena.buffer,
                        *firstVertex as u64 * std::mem::size_of::<WorldVertex>() as u64,
                        self.chunkIndexArena.buffer,
                        *firstIndex as u64 * std::mem::size_of::<u32>() as u64,
                    ),
                    ChunkStorage::Dedicated { vertexBuffer, indexBuffer } => (
                        vertexBuffer.buffer,
                        0,
                        indexBuffer.buffer,
                        0,
                    ),
                };
            self.pendingCopies[frameSlot].push(PendingBufferCopy {
                destination: vertexDestination,
                region: vk::BufferCopy {
                    src_offset: vertexOffset,
                    dst_offset: vertexDestinationOffset,
                    size: vertexBytes.len() as u64,
                },
                destinationAccess: vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
            });
            self.pendingCopies[frameSlot].push(PendingBufferCopy {
                destination: indexDestination,
                region: vk::BufferCopy {
                    src_offset: indexOffset,
                    dst_offset: indexDestinationOffset,
                    size: indexBytes.len() as u64,
                },
                destinationAccess: vk::AccessFlags::INDEX_READ,
            });
            stagingOffset = endOffset;

            let replacement = GpuChunk {
                storage,
                layerRanges: upload.layerRanges,
                vertexCount,
                indexCount,
                meshRevision: upload.meshRevision,
            };
            if let Some(previous) = self.chunks.insert(upload.key, replacement) {
                self.retire_chunk(frameSlot, previous);
            }
        }
        // Reference-renderer-style dynamic GPU residency. Each stream owns an
        // independent monotonic generation, so one changing particle does not
        // force entities, HUD or first-person data through staging again.
        let hasDynamicMeshes = [
            !frame.entityVertices.is_empty() && !frame.entityIndices.is_empty(),
            !frame.blockEntityVertices.is_empty() && !frame.blockEntityIndices.is_empty(),
            !frame.staticEntityVertices.is_empty() && !frame.staticEntityIndices.is_empty(),
            !frame.entityOverlayVertices.is_empty() && !frame.entityOverlayIndices.is_empty(),
            !frame.entityDepthVertices.is_empty() && !frame.entityDepthIndices.is_empty(),
            !frame.particleVertices.is_empty() && !frame.particleIndices.is_empty(),
            !frame.transparentParticleVertices.is_empty()
                && !frame.transparentParticleIndices.is_empty(),
            !frame.damageVertices.is_empty() && !frame.damageIndices.is_empty(),
            !frame.selectionVertices.is_empty() && !frame.selectionIndices.is_empty(),
            !frame.firstPersonVertices.is_empty() && !frame.firstPersonIndices.is_empty(),
            !frame.hudVertices.is_empty() && !frame.hudIndices.is_empty(),
        ]
        .into_iter()
        .any(|present| present);

        clear_empty_dynamic_mesh(
            &mut self.entityMeshes[frameSlot],
            frame.entityVertices.as_slice(),
            frame.entityIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.blockEntityMeshes[frameSlot],
            frame.blockEntityVertices.as_slice(),
            frame.blockEntityIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.staticEntityMeshes[frameSlot],
            frame.staticEntityVertices.as_slice(),
            frame.staticEntityIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.entityOverlayMeshes[frameSlot],
            frame.entityOverlayVertices.as_slice(),
            frame.entityOverlayIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.entityDepthMeshes[frameSlot],
            frame.entityDepthVertices.as_slice(),
            frame.entityDepthIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.particleMeshes[frameSlot],
            frame.particleVertices.as_slice(),
            frame.particleIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.transparentParticleMeshes[frameSlot],
            frame.transparentParticleVertices.as_slice(),
            frame.transparentParticleIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.damageMeshes[frameSlot],
            frame.damageVertices.as_slice(),
            frame.damageIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.selectionMeshes[frameSlot],
            frame.selectionVertices.as_slice(),
            frame.selectionIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.firstPersonMeshes[frameSlot],
            frame.firstPersonVertices.as_slice(),
            frame.firstPersonIndices.as_slice(),
        );
        clear_empty_dynamic_mesh(
            &mut self.hudMeshes[frameSlot],
            frame.hudVertices.as_slice(),
            frame.hudIndices.as_slice(),
        );

        if hasDynamicMeshes && uploadBytes > 0 {
            let staging = self.stagingBuffers[frameSlot]
                .as_ref()
                .context("world staging buffer missing for dynamic mesh uploads")?;
            let pendingCopies = &mut self.pendingCopies[frameSlot];
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.entityMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.entityVertices.as_slice(),
                frame.entityIndices.as_slice(),
                frame.entityMeshGeneration,
                "entity",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.blockEntityMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.blockEntityVertices.as_slice(),
                frame.blockEntityIndices.as_slice(),
                frame.blockEntityMeshGeneration,
                "block entity",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.staticEntityMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.staticEntityVertices.as_slice(),
                frame.staticEntityIndices.as_slice(),
                frame.staticEntityMeshGeneration,
                "static entity",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.entityOverlayMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.entityOverlayVertices.as_slice(),
                frame.entityOverlayIndices.as_slice(),
                frame.entityOverlayMeshGeneration,
                "entity overlay",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.entityDepthMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.entityDepthVertices.as_slice(),
                frame.entityDepthIndices.as_slice(),
                frame.entityDepthMeshGeneration,
                "entity depth",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.particleMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.particleVertices.as_slice(),
                frame.particleIndices.as_slice(),
                frame.particleMeshGeneration,
                "particle",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.transparentParticleMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.transparentParticleVertices.as_slice(),
                frame.transparentParticleIndices.as_slice(),
                frame.transparentParticleMeshGeneration,
                "transparent particle",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.damageMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.damageVertices.as_slice(),
                frame.damageIndices.as_slice(),
                frame.damageMeshGeneration,
                "block damage",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.selectionMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.selectionVertices.as_slice(),
                frame.selectionIndices.as_slice(),
                frame.selectionMeshGeneration,
                "selection/debug lines",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.firstPersonMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.firstPersonVertices.as_slice(),
                frame.firstPersonIndices.as_slice(),
                frame.firstPersonMeshGeneration,
                "first person",
            )?;
            stage_dynamic_mesh(
                device,
                memoryProperties,
                &mut self.hudMeshes[frameSlot],
                staging,
                pendingCopies,
                &mut stagingOffset,
                frame.hudVertices.as_slice(),
                frame.hudIndices.as_slice(),
                frame.hudMeshGeneration,
                "HUD",
            )?;
        } else if !hasDynamicMeshes {
            clear_dynamic_mesh(&mut self.entityMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.blockEntityMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.staticEntityMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.entityOverlayMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.entityDepthMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.particleMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.transparentParticleMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.damageMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.selectionMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.firstPersonMeshes[frameSlot]);
            clear_dynamic_mesh(&mut self.hudMeshes[frameSlot]);
        }

        self.prepare_chunk_indirect_commands(
            device,
            memoryProperties,
            frameSlot,
            frame,
        )?;

        if !hadDrawableChunks && !self.chunks.is_empty() && !self.loggedFirstChunkUpload {
            log::info!(
                "first RenderChunk device-local GPU buffers staged; resident chunks={}",
                self.chunks.len(),
            );
            self.loggedFirstChunkUpload = true;
        }
        Ok(())
    }

    fn prepare_chunk_indirect_commands(
        &mut self,
        device: &Device,
        memoryProperties: &vk::PhysicalDeviceMemoryProperties,
        frameSlot: usize,
        frame: &WorldRenderFrame,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            frameSlot < self.chunkIndirectBuffers.len()
                && frameSlot < self.chunkIndirectLayerOffsets.len()
                && frameSlot < self.chunkIndirectLayerCounts.len()
                && frameSlot < self.chunkIndirectSignatures.len(),
            "chunk indirect frame-slot index out of range",
        );
        let signature = self.chunk_indirect_signature(frame);
        if self.chunkIndirectSignatures[frameSlot] == Some(signature) {
            return Ok(());
        }
        self.chunkIndirectCommands.clear();
        self.chunkIndirectCommands
            .reserve(frame.visibleChunks.len().saturating_mul(4));
        let stride = std::mem::size_of::<ChunkIndirectCommand>() as u64;

        for layer in [
            BlockRenderLayer::Solid,
            BlockRenderLayer::CutoutMipped,
            BlockRenderLayer::Cutout,
            BlockRenderLayer::Translucent,
        ] {
            let layerIndex = layer.index();
            self.chunkIndirectLayerOffsets[frameSlot][layerIndex] =
                self.chunkIndirectCommands.len() as u64 * stride;
            let firstCommand = self.chunkIndirectCommands.len();
            if layer == BlockRenderLayer::Translucent {
                for visible in frame.visibleChunks.iter().rev() {
                    self.append_shared_chunk_command(visible.key, layerIndex)?;
                }
            } else {
                for visible in &frame.visibleChunks {
                    self.append_shared_chunk_command(visible.key, layerIndex)?;
                }
            }
            self.chunkIndirectLayerCounts[frameSlot][layerIndex] =
                u32::try_from(self.chunkIndirectCommands.len() - firstCommand)
                    .context("visible shared RenderChunk count exceeds u32")?;
        }

        let bytes = as_bytes(self.chunkIndirectCommands.as_slice());
        if bytes.is_empty() {
            self.chunkIndirectSignatures[frameSlot] = Some(signature);
            return Ok(());
        }
        ensure_indirect_capacity(
            device,
            memoryProperties,
            &mut self.chunkIndirectBuffers[frameSlot],
            bytes.len() as u64,
        )?;
        let indirect = self.chunkIndirectBuffers[frameSlot]
            .as_ref()
            .context("chunk indirect buffer missing after allocation")?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                indirect.mapped.as_ptr(),
                bytes.len(),
            );
        }
        self.chunkIndirectSignatures[frameSlot] = Some(signature);
        Ok(())
    }

    fn chunk_indirect_signature(&self, frame: &WorldRenderFrame) -> u64 {
        const OFFSET: u64 = 0xcbf29ce484222325;
        const PRIME: u64 = 0x100000001b3;
        let mix = |hash: &mut u64, value: u64| {
            for byte in value.to_le_bytes() {
                *hash ^= u64::from(byte);
                *hash = hash.wrapping_mul(PRIME);
            }
        };

        let mut hash = OFFSET;
        mix(&mut hash, frame.visibleChunks.len() as u64);
        for visible in &frame.visibleChunks {
            mix(&mut hash, visible.key.x as u32 as u64);
            mix(&mut hash, visible.key.y as u32 as u64);
            mix(&mut hash, visible.key.z as u32 as u64);
            let Some(chunk) = self.chunks.get(&visible.key) else {
                mix(&mut hash, u64::MAX);
                continue;
            };
            // The indirect draw command depends on allocation offsets and
            // layer ranges, not on vertex/index contents. A transparency
            // resort changes meshRevision and index bytes but retains this
            // command stream exactly.
            match &chunk.storage {
                ChunkStorage::Shared {
                    firstVertex,
                    vertexCount,
                    firstIndex,
                    indexCount,
                } => {
                    mix(&mut hash, 1);
                    mix(&mut hash, u64::from(*firstVertex));
                    mix(&mut hash, u64::from(*vertexCount));
                    mix(&mut hash, u64::from(*firstIndex));
                    mix(&mut hash, u64::from(*indexCount));
                }
                ChunkStorage::Dedicated { .. } => mix(&mut hash, 2),
            }
            for range in chunk.layerRanges {
                mix(&mut hash, u64::from(range.firstIndex));
                mix(&mut hash, u64::from(range.indexCount));
            }
        }
        hash
    }

    fn append_shared_chunk_command(
        &mut self,
        key: RenderChunkKey,
        layerIndex: usize,
    ) -> anyhow::Result<()> {
        let Some(chunk) = self.chunks.get(&key) else { return Ok(()); };
        let range = chunk.layerRanges[layerIndex];
        if range.indexCount == 0 { return Ok(()); }
        let ChunkStorage::Shared { firstVertex, firstIndex, .. } = &chunk.storage else {
            return Ok(());
        };
        let globalFirstIndex = (*firstIndex)
            .checked_add(range.firstIndex)
            .context("shared RenderChunk index offset overflow")?;
        self.chunkIndirectCommands.push(ChunkIndirectCommand {
            index_count: range.indexCount,
            instance_count: 1,
            first_index: globalFirstIndex,
            vertex_offset: i32::try_from(*firstVertex)
                .context("shared RenderChunk vertex offset exceeds i32")?,
            first_instance: 0,
        });
        Ok(())
    }

    fn record_shared_chunk_run(
        &self,
        device: &Device,
        commandBuffer: vk::CommandBuffer,
        frameSlot: usize,
        layerIndex: usize,
        firstCommand: u32,
        commandCount: u32,
    ) -> (u64, u64) {
        if commandCount == 0 {
            return (0, 0);
        }
        let Some(indirect) = self.chunkIndirectBuffers[frameSlot].as_ref() else {
            return (0, 0);
        };
        let stride = std::mem::size_of::<ChunkIndirectCommand>() as u32;
        let offset = self.chunkIndirectLayerOffsets[frameSlot][layerIndex]
            + firstCommand as u64 * stride as u64;
        let first = (offset / stride as u64) as usize;
        let last = first.saturating_add(commandCount as usize);
        let submittedIndices = self
            .chunkIndirectCommands
            .get(first..last)
            .unwrap_or(&[])
            .iter()
            .map(|command| command.index_count as u64)
            .sum::<u64>();

        unsafe {
            device.cmd_bind_vertex_buffers(
                commandBuffer,
                0,
                &[self.chunkVertexArena.buffer],
                &[0],
            );
            device.cmd_bind_index_buffer(
                commandBuffer,
                self.chunkIndexArena.buffer,
                0,
                vk::IndexType::UINT32,
            );
            if self.multiDrawIndirect {
                device.cmd_draw_indexed_indirect(
                    commandBuffer,
                    indirect.buffer,
                    offset,
                    commandCount,
                    stride,
                );
                (1, submittedIndices)
            } else {
                for command in 0..commandCount {
                    device.cmd_draw_indexed_indirect(
                        commandBuffer,
                        indirect.buffer,
                        offset + command as u64 * stride as u64,
                        1,
                        stride,
                    );
                }
                (commandCount as u64, submittedIndices)
            }
        }
    }

    /// Records one Minecraft block layer in the exact `RenderGlobal` order.
    /// Consecutive shared-arena chunks collapse into one indirect submission;
    /// a rare dedicated fallback flushes the run, draws in place, then resumes
    /// batching. This retains far-to-near translucent ordering even when an
    /// arena is exhausted instead of moving all fallback chunks to the end.
    fn record_chunk_layer<I>(
        &self,
        device: &Device,
        commandBuffer: vk::CommandBuffer,
        frameSlot: usize,
        layerIndex: usize,
        keys: I,
    ) -> (u64, u64, u64)
    where
        I: IntoIterator<Item = RenderChunkKey>,
    {
        let mut submitCalls = 0_u64;
        let mut logicalRanges = 0_u64;
        let mut submittedIndices = 0_u64;
        let mut sharedCursor = 0_u32;
        let mut runStart = 0_u32;
        let mut runCount = 0_u32;

        for key in keys {
            let Some(chunk) = self.chunks.get(&key) else {
                continue;
            };
            let range = chunk.layerRanges[layerIndex];
            if range.indexCount == 0 {
                continue;
            }

            match &chunk.storage {
                ChunkStorage::Shared { .. } => {
                    if runCount == 0 {
                        runStart = sharedCursor;
                    }
                    runCount = runCount.saturating_add(1);
                    sharedCursor = sharedCursor.saturating_add(1);
                    logicalRanges = logicalRanges.saturating_add(1);
                }
                ChunkStorage::Dedicated {
                    vertexBuffer,
                    indexBuffer,
                } => {
                    if runCount > 0 {
                        let (calls, indices) = self.record_shared_chunk_run(
                            device,
                            commandBuffer,
                            frameSlot,
                            layerIndex,
                            runStart,
                            runCount,
                        );
                        submitCalls = submitCalls.saturating_add(calls);
                        submittedIndices = submittedIndices.saturating_add(indices);
                        runCount = 0;
                    }
                    unsafe {
                        device.cmd_bind_vertex_buffers(
                            commandBuffer,
                            0,
                            &[vertexBuffer.buffer],
                            &[0],
                        );
                        device.cmd_bind_index_buffer(
                            commandBuffer,
                            indexBuffer.buffer,
                            0,
                            vk::IndexType::UINT32,
                        );
                        device.cmd_draw_indexed(
                            commandBuffer,
                            range.indexCount,
                            1,
                            range.firstIndex,
                            0,
                            0,
                        );
                    }
                    submitCalls = submitCalls.saturating_add(1);
                    logicalRanges = logicalRanges.saturating_add(1);
                    submittedIndices = submittedIndices.saturating_add(range.indexCount as u64);
                }
            }
        }

        if runCount > 0 {
            let (calls, indices) = self.record_shared_chunk_run(
                device,
                commandBuffer,
                frameSlot,
                layerIndex,
                runStart,
                runCount,
            );
            submitCalls = submitCalls.saturating_add(calls);
            submittedIndices = submittedIndices.saturating_add(indices);
        }

        debug_assert_eq!(
            sharedCursor,
            self.chunkIndirectLayerCounts[frameSlot][layerIndex],
            "shared indirect command stream diverged from visible RenderChunk order",
        );
        (submitCalls, logicalRanges, submittedIndices)
    }

    fn retire_chunk(&mut self, frameSlot: usize, chunk: GpuChunk) {
        self.retiredChunkStorage[frameSlot].push(chunk.storage);
    }

    pub fn record(
        &mut self,
        device: &Device,
        commandBuffer: vk::CommandBuffer,
        frameSlot: usize,
        imageIndex: usize,
        extent: vk::Extent2D,
        frame: &WorldRenderFrame,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(imageIndex < self.framebuffers.len(), "world framebuffer index out of range");
        anyhow::ensure!(frameSlot < self.pendingCopies.len(), "world copy frame-slot index out of range");
        anyhow::ensure!(frameSlot < self.descriptorSets.len() && frameSlot < self.pendingTextureUploads.len(), "world texture frame-slot index out of range");
        unsafe {
            device
                .reset_command_buffer(commandBuffer, vk::CommandBufferResetFlags::empty())
                .context("failed resetting Vulkan world command buffer")?;
        }
        let beginInfo = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            device
                .begin_command_buffer(commandBuffer, &beginInfo)
                .context("failed beginning Vulkan world command buffer")?;
        }

        if let Some(pending) = self.pendingTextureUploads[frameSlot].take() {
            let toTransfer = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(pending.image)
                .subresource_range(color_subresource());
            unsafe {
                device.cmd_pipeline_barrier(
                    commandBuffer,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[], &[], &[toTransfer],
                );
                let region = vk::BufferImageCopy::default()
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
                    .image_extent(vk::Extent3D { width: pending.width, height: pending.height, depth: 1 });
                device.cmd_copy_buffer_to_image(
                    commandBuffer,
                    pending.staging.buffer,
                    pending.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[region],
                );
                let toShader = vk::ImageMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(pending.image)
                    .subresource_range(color_subresource());
                device.cmd_pipeline_barrier(
                    commandBuffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[], &[], &[toShader],
                );
            }
            // The staging allocation remains alive until this frame-slot fence
            // signals on its next reuse.
            self.retiredBuffers[frameSlot].push(pending.staging);
        }

        let pendingCopies = std::mem::take(&mut self.pendingCopies[frameSlot]);
        if !pendingCopies.is_empty() {
            let staging = self.stagingBuffers[frameSlot]
                .as_ref()
                .context("world staging buffer missing while recording mesh copies")?;
            // Group copies by destination buffer so shared arenas receive one copy
            // command and one barrier per frame rather than per RenderChunk.
            // Grouping is safe
            // because every region references the same frame-slot staging
            // buffer and chunk allocations are disjoint.
            let mut groupedCopies = HashMap::<
                vk::Buffer,
                (Vec<vk::BufferCopy>, vk::AccessFlags),
            >::new();
            for copy in pendingCopies {
                let entry = groupedCopies
                    .entry(copy.destination)
                    .or_insert_with(|| (Vec::new(), vk::AccessFlags::empty()));
                entry.0.push(copy.region);
                entry.1 |= copy.destinationAccess;
            }
            unsafe {
                for (&destination, (regions, _)) in &groupedCopies {
                    device.cmd_copy_buffer(
                        commandBuffer,
                        staging.buffer,
                        destination,
                        regions,
                    );
                }
                let barriers = groupedCopies
                    .iter()
                    .map(|(&destination, (_, destinationAccess))| {
                        vk::BufferMemoryBarrier::default()
                            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                            .dst_access_mask(*destinationAccess)
                            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .buffer(destination)
                            .offset(0)
                            .size(vk::WHOLE_SIZE)
                    })
                    .collect::<Vec<_>>();
                device.cmd_pipeline_barrier(
                    commandBuffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::VERTEX_INPUT,
                    vk::DependencyFlags::empty(),
                    &[],
                    &barriers,
                    &[],
                );
            }
        }

        let clearValues = [
            vk::ClearValue {
                color: vk::ClearColorValue { float32: frame.clearColor },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
            },
        ];
        let renderArea = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };
        let renderPassInfo = vk::RenderPassBeginInfo::default()
            .render_pass(self.renderPass)
            .framebuffer(self.framebuffers[imageIndex])
            .render_area(renderArea)
            .clear_values(&clearValues);
        unsafe {
            device.cmd_begin_render_pass(
                commandBuffer,
                &renderPassInfo,
                vk::SubpassContents::INLINE,
            );
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: extent.width as f32,
                height: extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            device.cmd_set_viewport(commandBuffer, 0, &[viewport]);
            device.cmd_set_scissor(commandBuffer, 0, &[renderArea]);
            device.cmd_bind_descriptor_sets(
                commandBuffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelineLayout,
                0,
                &[self.descriptorSets[frameSlot]],
                &[],
            );

            let mut drawCount = 0_usize;
            let mut submittedIndices = 0_u64;
            let mut logicalChunkRanges = 0_u64;

            // `RenderGlobal#renderSky` runs before terrain. Its vertices share
            // the dynamic overlay buffer, but use a separate 512-block sky
            // projection and bypass both lightmap and fog.
            if frame.skyAlphaIndexCount > 0 || frame.skyCelestialIndexCount > 0 {
                if let Some(mesh) = self.entityOverlayMeshes[frameSlot]
                    .as_ref()
                    .filter(|mesh| mesh.indexCount > 0)
                {
                    device.cmd_bind_vertex_buffers(commandBuffer, 0, &[mesh.vertexBuffer.buffer], &[0]);
                    device.cmd_bind_index_buffer(commandBuffer, mesh.indexBuffer.buffer, 0, vk::IndexType::UINT32);
                    if frame.skyAlphaIndexCount > 0 {
                        device.cmd_bind_pipeline(
                            commandBuffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.hudPipeline,
                        );
                        device.cmd_push_constants(
                            commandBuffer,
                            self.pipelineLayout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0,
                            struct_as_bytes(&frame.skyPushConstants),
                        );
                        device.cmd_draw_indexed(
                            commandBuffer,
                            frame.skyAlphaIndexCount,
                            1,
                            0,
                            0,
                            0,
                        );
                        drawCount += 1;
                        submittedIndices += frame.skyAlphaIndexCount as u64;
                    }
                    if frame.skyCelestialIndexCount > 0 {
                        device.cmd_bind_pipeline(
                            commandBuffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.skyCelestialPipeline,
                        );
                        device.cmd_push_constants(
                            commandBuffer,
                            self.pipelineLayout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0,
                            struct_as_bytes(&frame.skyPushConstants),
                        );
                        device.cmd_draw_indexed(
                            commandBuffer,
                            frame.skyCelestialIndexCount,
                            1,
                            frame.skyAlphaIndexCount,
                            0,
                            0,
                        );
                        drawCount += 1;
                        submittedIndices += frame.skyCelestialIndexCount as u64;
                    }
                }
            }

            // EntityRenderer renders terrain in the exact vanilla order:
            // SOLID, CUTOUT_MIPPED, CUTOUT, then TRANSLUCENT. Opaque/cutout
            // layers write depth and do not blend. The fragment shader receives
            // the current alpha-test threshold through the unused fourth fog
            // parameter so the push-constant layout stays source-compatible.
            device.cmd_bind_pipeline(
                commandBuffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.opaquePipeline,
            );
            for layer in [
                BlockRenderLayer::Solid,
                BlockRenderLayer::CutoutMipped,
                BlockRenderLayer::Cutout,
            ] {
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = if layer == BlockRenderLayer::Solid {
                    -1.0
                } else {
                    0.1
                };
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                let (layerSubmitCalls, layerRanges, layerIndices) = self.record_chunk_layer(
                    device,
                    commandBuffer,
                    frameSlot,
                    layer.index(),
                    frame.visibleChunks.iter().map(|visible| visible.key),
                );
                drawCount = drawCount.saturating_add(layerSubmitCalls as usize);
                logicalChunkRanges = logicalChunkRanges.saturating_add(layerRanges);
                submittedIndices = submittedIndices.saturating_add(layerIndices);
            }

            // `RenderGlobal#renderEntities` preserves source order across
            // ordinary entities and TileEntityRendererDispatcher. Immutable
            // TESR/hanging meshes now own independent resident buffers, while
            // this command list interleaves them with the dynamic stream.
            if !frame.entityDrawRanges.is_empty() {
                device.cmd_bind_pipeline(
                    commandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.entityPipeline,
                );
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = 0.1;
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                let mut boundMesh = None;
                // The range plan is emitted in RenderManager.renderEntityStatic /
        // TileEntityRendererDispatcher source order by VulkanWorldRenderer.
        for range in &frame.entityDrawRanges {
                    let mesh = match range.mesh {
                        WorldEntityMeshKind::Dynamic => self.entityMeshes[frameSlot].as_ref(),
                        WorldEntityMeshKind::BlockEntities => {
                            self.blockEntityMeshes[frameSlot].as_ref()
                        }
                        WorldEntityMeshKind::StaticEntities => {
                            self.staticEntityMeshes[frameSlot].as_ref()
                        }
                    };
                    let Some(mesh) = mesh.filter(|mesh| {
                        range.indexCount > 0
                            && range.firstIndex.saturating_add(range.indexCount)
                                <= mesh.indexCount
                    }) else {
                        continue;
                    };
                    if boundMesh != Some(range.mesh) {
                        device.cmd_bind_vertex_buffers(
                            commandBuffer,
                            0,
                            &[mesh.vertexBuffer.buffer],
                            &[0],
                        );
                        device.cmd_bind_index_buffer(
                            commandBuffer,
                            mesh.indexBuffer.buffer,
                            0,
                            vk::IndexType::UINT32,
                        );
                        boundMesh = Some(range.mesh);
                    }
                    device.cmd_draw_indexed(
                        commandBuffer,
                        range.indexCount,
                        1,
                        range.firstIndex,
                        0,
                        0,
                    );
                    drawCount += 1;
                    submittedIndices += range.indexCount as u64;
                }
            }

            // RenderTntMinecart and TileEntityEndPortal share one dynamic
            // overlay buffer but retain their distinct OpenGL blend profiles.
            if let Some(entityOverlayMesh) = self.entityOverlayMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[entityOverlayMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    entityOverlayMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                for range in &frame.entityOverlayDrawRanges {
                    let (pipeline, textureSentinel, unlit, noFog, blackFog) = match range.pipeline {
                        EntityOverlayPipelineKind::ArmorGlint => {
                            // LayerArmorBase.renderEnchantedGlint: GL_EQUAL,
                            // depthMask(false), SRC_COLOR/ONE, lighting off and
                            // EntityRenderer.setupFogColor(true).
                            (self.firstPersonGlintPipeline, 0.1, true, false, true)
                        }
                        EntityOverlayPipelineKind::TntFlash => {
                            (self.entityOverlayPipeline, -2.0, false, false, false)
                        }
                        EntityOverlayPipelineKind::BeaconCore => {
                            // TileEntityBeaconRenderer disables lighting,
                            // fog and blending for the opaque inner shaft.
                            (self.beaconCorePipeline, 0.1, true, true, false)
                        }
                        EntityOverlayPipelineKind::BeaconGlow => {
                            // The outer shaft uses SRC_ALPHA /
                            // ONE_MINUS_SRC_ALPHA with depth writes disabled.
                            (self.beaconGlowPipeline, 0.1, true, true, false)
                        }
                        EntityOverlayPipelineKind::EndPortalAlpha => {
                            (self.entityPipeline, 0.1, true, false, false)
                        }
                        EntityOverlayPipelineKind::EndPortalAdditive => {
                            (self.endPortalAdditivePipeline, 0.1, true, false, true)
                        }
                    };
                    device.cmd_bind_pipeline(
                        commandBuffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline,
                    );
                    let mut constants = frame.pushConstants;
                    constants.fogParameters[3] = textureSentinel;
                    if noFog {
                        // Sentinel > 10 selects the shader's unlit/no-fog
                        // branch used for GUI-space and explicit vanilla
                        // renderers that disable both lightmap and fog.
                        constants.lightmapParameters[3] = 99.0;
                    } else if unlit {
                        // Shader dimension 98 is the established unlit/fogged
                        // path used for passes where MCP disables lighting.
                        constants.lightmapParameters[3] = 98.0;
                    }
                    if blackFog {
                        // EntityRenderer#setupFogColor(true), active from
                        // portal layer j >= 1 until the TESR finishes.
                        constants.fogColor = [0.0, 0.0, 0.0, 1.0];
                    }
                    device.cmd_push_constants(
                        commandBuffer,
                        self.pipelineLayout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        struct_as_bytes(&constants),
                    );
                    device.cmd_draw_indexed(
                        commandBuffer,
                        range.indexCount,
                        1,
                        range.firstIndex,
                        0,
                        0,
                    );
                    drawCount += 1;
                    submittedIndices += range.indexCount as u64;
                }
            }

            // `RenderGlobal#renderEntities` then iterates the multipass list.
            // Boat's `ModelBoat#renderMultipass` has the same transform and
            // texture/alpha-test state, but masks every color channel while
            // retaining depth test and depth writes. This produces the
            // original no-water depth occluder without a visible polygon.
            if let Some(entityDepthMesh) = self.entityDepthMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_pipeline(
                    commandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.entityDepthPipeline,
                );
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = 0.1;
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[entityDepthMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    entityDepthMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                device.cmd_draw_indexed(
                    commandBuffer,
                    entityDepthMesh.indexCount,
                    1,
                    0,
                    0,
                    0,
                );
                drawCount += 1;
                submittedIndices += entityDepthMesh.indexCount as u64;
            }

            // RenderGlobal.drawSelectionBox: source-alpha black lines, depth
            // test retained, depth writes disabled, texture disabled and the
            // selected AABB expanded by 0.002 on every axis.
            if let Some(selectionMesh) = self.selectionMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_pipeline(
                    commandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.selectionPipeline,
                );
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = -2.0;
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[selectionMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    selectionMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                device.cmd_draw_indexed(commandBuffer, selectionMesh.indexCount, 1, 0, 0, 0);
                drawCount += 1;
                submittedIndices += selectionMesh.indexCount as u64;
            }

            // RenderGlobal.drawBlockDamageTexture runs after the selection
            // box. The mesh already contains BakedQuadRetextured UVs for the
            // selected destroy_stage_0..9 sprite; the dedicated pipeline
            // supplies vanilla's multiplicative blend and polygon offset.
            if let Some(damageMesh) = self.damageMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_pipeline(
                    commandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.damagePipeline,
                );
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = 0.1;
                // EntityRenderer disables the lightmap before entities and
                // does not enable it again until lit particles. Damage remains
                // fogged, so the shader's dedicated unlit-world sentinel is
                // used instead of the HUD's no-lightmap/no-fog sentinel.
                constants.lightmapParameters[3] = 98.0;
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[damageMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    damageMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                device.cmd_draw_indexed(commandBuffer, damageMesh.indexCount, 1, 0, 0, 0);
                drawCount += 1;
                submittedIndices += damageMesh.indexCount as u64;
            }

            // ParticleManager iterates `isTransparent()` first and uses
            // depthMask(false) while retaining depth testing and alpha blend.
            if let Some(particleMesh) = self.transparentParticleMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_pipeline(
                    commandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.translucentPipeline,
                );
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = 0.003921569;
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[particleMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    particleMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                device.cmd_draw_indexed(commandBuffer, particleMesh.indexCount, 1, 0, 0, 0);
                drawCount += 1;
                submittedIndices += particleMesh.indexCount as u64;
            }

            // ParticleManager renders the non-transparent TextureMap queue
            // with alpha blending, depth writes and the normal back-face cull
            // state. ParticleDigging uses alphaFunc(GREATER, 1/255).
            if let Some(particleMesh) = self.particleMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_pipeline(
                    commandBuffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    // This pipeline has the exact Alpha + depth-write +
                    // BACK-cull raster state required by ParticleManager's
                    // non-transparent terrain-particle queue.
                    self.firstPersonPipeline,
                );
                let mut constants = frame.pushConstants;
                constants.fogParameters[3] = 0.003921569;
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&constants),
                );
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[particleMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    particleMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                device.cmd_draw_indexed(commandBuffer, particleMesh.indexCount, 1, 0, 0, 0);
                drawCount += 1;
                submittedIndices += particleMesh.indexCount as u64;
            }

            // Vanilla traverses prepared RenderChunks in reverse for the
            // translucent layer and disables depth writes while keeping depth
            // testing enabled. Batch 108 also applies BufferBuilder's stable
            // far-to-near quad sort inside each RenderChunk.
            device.cmd_bind_pipeline(
                commandBuffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.translucentPipeline,
            );
            let mut constants = frame.pushConstants;
            constants.fogParameters[3] = 0.1;
            device.cmd_push_constants(
                commandBuffer,
                self.pipelineLayout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                struct_as_bytes(&constants),
            );
            let translucentLayer = BlockRenderLayer::Translucent.index();
            let (layerSubmitCalls, layerRanges, layerIndices) = self.record_chunk_layer(
                device,
                commandBuffer,
                frameSlot,
                translucentLayer,
                frame.visibleChunks.iter().rev().map(|visible| visible.key),
            );
            drawCount = drawCount.saturating_add(layerSubmitCalls as usize);
            logicalChunkRanges = logicalChunkRanges.saturating_add(layerRanges);
            submittedIndices = submittedIndices.saturating_add(layerIndices);

            // EntityRenderer.renderWorldPass clears only the depth buffer
            // immediately before `renderHand`. First-person items therefore
            // cannot be occluded by terrain, but main/off-hand models still
            // depth-test against each other exactly like the OpenGL pass.
            if let Some(firstPersonMesh) = self.firstPersonMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                let depthClear = vk::ClearAttachment {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    color_attachment: 0,
                    clear_value: vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
                    },
                    ..Default::default()
                };
                let clearRect = vk::ClearRect {
                    rect: renderArea,
                    base_array_layer: 0,
                    layer_count: 1,
                };
                device.cmd_clear_attachments(commandBuffer, &[depthClear], &[clearRect]);
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&frame.firstPersonPushConstants),
                );
                device.cmd_bind_vertex_buffers(
                    commandBuffer,
                    0,
                    &[firstPersonMesh.vertexBuffer.buffer],
                    &[0],
                );
                device.cmd_bind_index_buffer(
                    commandBuffer,
                    firstPersonMesh.indexBuffer.buffer,
                    0,
                    vk::IndexType::UINT32,
                );
                for range in &frame.firstPersonDrawRanges {
                    if range.indexCount == 0 { continue; }
                    let pipeline = match range.pipeline {
                        FirstPersonPipelineKind::Alpha => self.firstPersonPipeline,
                        FirstPersonPipelineKind::Fire => self.firstPersonFirePipeline,
                        FirstPersonPipelineKind::Glint => self.firstPersonGlintPipeline,
                    };
                    device.cmd_bind_pipeline(
                        commandBuffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline,
                    );
                    device.cmd_draw_indexed(
                        commandBuffer,
                        range.indexCount,
                        1,
                        range.firstIndex,
                        0,
                        0,
                    );
                    drawCount += 1;
                    submittedIndices += range.indexCount as u64;
                }
            }

            // `GuiIngame.renderGameOverlay` executes after the world render.
            // The hotbar uses normal source-alpha blending, while the 1.12.2
            // crosshair uses the inversion blend profile. Both are drawn with
            // depth testing disabled in scaled GUI coordinates.
            if let Some(hudMesh) = self.hudMeshes[frameSlot]
                .as_ref()
                .filter(|mesh| mesh.indexCount > 0)
            {
                device.cmd_bind_vertex_buffers(commandBuffer, 0, &[hudMesh.vertexBuffer.buffer], &[0]);
                device.cmd_bind_index_buffer(commandBuffer, hudMesh.indexBuffer.buffer, 0, vk::IndexType::UINT32);
                device.cmd_push_constants(
                    commandBuffer,
                    self.pipelineLayout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    struct_as_bytes(&frame.hudPushConstants),
                );
                for range in &frame.hudDrawRanges {
                    if range.indexCount == 0 { continue; }
                    let pipeline = match range.pipeline {
                        HudPipelineKind::Alpha => self.hudPipeline,
                        HudPipelineKind::Crosshair => self.crosshairPipeline,
                        HudPipelineKind::Glint => self.glintPipeline,
                    };
                    device.cmd_bind_pipeline(
                        commandBuffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline,
                    );
                    device.cmd_draw_indexed(
                        commandBuffer,
                        range.indexCount,
                        1,
                        range.firstIndex,
                        0,
                        0,
                    );
                    drawCount += 1;
                    submittedIndices += range.indexCount as u64;
                }
            }

            if drawCount > 0 && !self.loggedFirstDraw {
                log::info!("first Vulkan world draw recorded: {drawCount} layer draws");
                self.loggedFirstDraw = true;
            }
            self.performanceFrames = self.performanceFrames.saturating_add(1);
            let performanceElapsed = self.performanceLogStarted.elapsed();
            if performanceElapsed >= Duration::from_secs(5) {
                let sharedVertexUsed = CHUNK_VERTEX_ARENA_BYTES.saturating_sub(
                    self.chunkVertexRanges.free_count()
                        .saturating_mul(std::mem::size_of::<WorldVertex>() as u64),
                );
                let sharedIndexUsed = CHUNK_INDEX_ARENA_BYTES.saturating_sub(
                    self.chunkIndexRanges.free_count()
                        .saturating_mul(std::mem::size_of::<u32>() as u64),
                );
                let mut residentBytes = sharedVertexUsed.saturating_add(sharedIndexUsed);
                for chunk in self.chunks.values() {
                    if let ChunkStorage::Dedicated { vertexBuffer, indexBuffer } = &chunk.storage {
                        residentBytes = residentBytes
                            .saturating_add(vertexBuffer.size)
                            .saturating_add(indexBuffer.size);
                    }
                }
                if let Some(entityMesh) = self.entityMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(entityMesh.vertexBuffer.size)
                        .saturating_add(entityMesh.indexBuffer.size);
                }
                if let Some(blockEntityMesh) = self.blockEntityMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(blockEntityMesh.vertexBuffer.size)
                        .saturating_add(blockEntityMesh.indexBuffer.size);
                }
                if let Some(staticEntityMesh) = self.staticEntityMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(staticEntityMesh.vertexBuffer.size)
                        .saturating_add(staticEntityMesh.indexBuffer.size);
                }
                if let Some(entityOverlayMesh) = self.entityOverlayMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(entityOverlayMesh.vertexBuffer.size)
                        .saturating_add(entityOverlayMesh.indexBuffer.size);
                }
                if let Some(entityDepthMesh) = self.entityDepthMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(entityDepthMesh.vertexBuffer.size)
                        .saturating_add(entityDepthMesh.indexBuffer.size);
                }
                if let Some(particleMesh) = self.particleMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(particleMesh.vertexBuffer.size)
                        .saturating_add(particleMesh.indexBuffer.size);
                }
                if let Some(particleMesh) = self.transparentParticleMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(particleMesh.vertexBuffer.size)
                        .saturating_add(particleMesh.indexBuffer.size);
                }
                if let Some(damageMesh) = self.damageMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(damageMesh.vertexBuffer.size)
                        .saturating_add(damageMesh.indexBuffer.size);
                }
                if let Some(selectionMesh) = self.selectionMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(selectionMesh.vertexBuffer.size)
                        .saturating_add(selectionMesh.indexBuffer.size);
                }
                if let Some(firstPersonMesh) = self.firstPersonMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(firstPersonMesh.vertexBuffer.size)
                        .saturating_add(firstPersonMesh.indexBuffer.size);
                }
                if let Some(hudMesh) = self.hudMeshes[frameSlot].as_ref() {
                    residentBytes = residentBytes
                        .saturating_add(hudMesh.vertexBuffer.size)
                        .saturating_add(hudMesh.indexBuffer.size);
                }
                let framesPerSecond =
                    self.performanceFrames as f64 / performanceElapsed.as_secs_f64().max(0.001);
                log::info!(
                    "World GPU workload: {:.1} fps, visible_chunks={}, submit_calls={}, logical_chunk_ranges={}, triangles={}, remote_players={}, non_player_entities={}, resident_chunks={}, shared_arena={:.1}/{:.1} MiB, device_local_mesh={:.1} MiB",
                    framesPerSecond,
                    frame.visibleChunks.len(),
                    drawCount,
                    logicalChunkRanges,
                    submittedIndices / 3,
                    frame.renderedRemotePlayers,
                    frame.renderedNonPlayerEntities,
                    self.chunks.len(),
                    (sharedVertexUsed + sharedIndexUsed) as f64 / (1024.0 * 1024.0),
                    (CHUNK_VERTEX_ARENA_BYTES + CHUNK_INDEX_ARENA_BYTES) as f64
                        / (1024.0 * 1024.0),
                    residentBytes as f64 / (1024.0 * 1024.0),
                );
                self.performanceFrames = 0;
                self.performanceLogStarted = Instant::now();
            }
            device.cmd_end_render_pass(commandBuffer);
            device
                .end_command_buffer(commandBuffer)
                .context("failed ending Vulkan world command buffer")?;
        }
        Ok(())
    }

    pub fn destroy_swapchain_resources(&mut self, device: &Device) {
        unsafe {
            for framebuffer in self.framebuffers.drain(..) {
                device.destroy_framebuffer(framebuffer, None);
            }
            if self.opaquePipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.opaquePipeline, None);
                self.opaquePipeline = vk::Pipeline::null();
            }
            if self.entityPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.entityPipeline, None);
                self.entityPipeline = vk::Pipeline::null();
            }
            if self.entityOverlayPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.entityOverlayPipeline, None);
                self.entityOverlayPipeline = vk::Pipeline::null();
            }
            if self.endPortalAdditivePipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.endPortalAdditivePipeline, None);
                self.endPortalAdditivePipeline = vk::Pipeline::null();
            }
            if self.beaconCorePipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.beaconCorePipeline, None);
                self.beaconCorePipeline = vk::Pipeline::null();
            }
            if self.beaconGlowPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.beaconGlowPipeline, None);
                self.beaconGlowPipeline = vk::Pipeline::null();
            }
            if self.skyCelestialPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.skyCelestialPipeline, None);
                self.skyCelestialPipeline = vk::Pipeline::null();
            }
            if self.entityDepthPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.entityDepthPipeline, None);
                self.entityDepthPipeline = vk::Pipeline::null();
            }
            if self.damagePipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.damagePipeline, None);
                self.damagePipeline = vk::Pipeline::null();
            }
            if self.selectionPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.selectionPipeline, None);
                self.selectionPipeline = vk::Pipeline::null();
            }
            if self.firstPersonPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.firstPersonPipeline, None);
                self.firstPersonPipeline = vk::Pipeline::null();
            }
            if self.firstPersonFirePipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.firstPersonFirePipeline, None);
                self.firstPersonFirePipeline = vk::Pipeline::null();
            }
            if self.firstPersonGlintPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.firstPersonGlintPipeline, None);
                self.firstPersonGlintPipeline = vk::Pipeline::null();
            }
            if self.translucentPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.translucentPipeline, None);
                self.translucentPipeline = vk::Pipeline::null();
            }
            if self.hudPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.hudPipeline, None);
                self.hudPipeline = vk::Pipeline::null();
            }
            if self.crosshairPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.crosshairPipeline, None);
                self.crosshairPipeline = vk::Pipeline::null();
            }
            if self.glintPipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.glintPipeline, None);
                self.glintPipeline = vk::Pipeline::null();
            }
            if self.renderPass != vk::RenderPass::null() {
                device.destroy_render_pass(self.renderPass, None);
                self.renderPass = vk::RenderPass::null();
            }
            for view in self.depthViews.drain(..) {
                device.destroy_image_view(view, None);
            }
            for image in self.depthImages.drain(..) {
                device.destroy_image(image, None);
            }
            for memory in self.depthMemories.drain(..) {
                device.free_memory(memory, None);
            }
            for view in self.swapchainImageViews.drain(..) {
                device.destroy_image_view(view, None);
            }
        }
    }

    pub fn destroy(&mut self, device: &Device) {
        self.destroy_swapchain_resources(device);
        for (_, chunk) in self.chunks.drain() {
            if let ChunkStorage::Dedicated { vertexBuffer, indexBuffer } = chunk.storage {
                destroy_buffer(device, Some(vertexBuffer));
                destroy_buffer(device, Some(indexBuffer));
            }
        }
        for retired in &mut self.retiredChunkStorage {
            for storage in retired.drain(..) {
                if let ChunkStorage::Dedicated { vertexBuffer, indexBuffer } = storage {
                    destroy_buffer(device, Some(vertexBuffer));
                    destroy_buffer(device, Some(indexBuffer));
                }
            }
        }
        for indirect in &mut self.chunkIndirectBuffers {
            destroy_staging_buffer(device, indirect.take());
        }
        self.chunkIndirectCommands.clear();
        self.chunkIndirectSignatures.clear();
        destroy_buffer(device, Some(std::mem::replace(
            &mut self.chunkVertexArena,
            GpuBuffer { buffer: vk::Buffer::null(), memory: vk::DeviceMemory::null(), size: 0 },
        )));
        destroy_buffer(device, Some(std::mem::replace(
            &mut self.chunkIndexArena,
            GpuBuffer { buffer: vk::Buffer::null(), memory: vk::DeviceMemory::null(), size: 0 },
        )));
        for entityMesh in &mut self.entityMeshes {
            if let Some(mesh) = entityMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for blockEntityMesh in &mut self.blockEntityMeshes {
            if let Some(mesh) = blockEntityMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for staticEntityMesh in &mut self.staticEntityMeshes {
            if let Some(mesh) = staticEntityMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for entityOverlayMesh in &mut self.entityOverlayMeshes {
            if let Some(mesh) = entityOverlayMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for entityDepthMesh in &mut self.entityDepthMeshes {
            if let Some(mesh) = entityDepthMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for particleMesh in &mut self.particleMeshes {
            if let Some(mesh) = particleMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for particleMesh in &mut self.transparentParticleMeshes {
            if let Some(mesh) = particleMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for damageMesh in &mut self.damageMeshes {
            if let Some(mesh) = damageMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for selectionMesh in &mut self.selectionMeshes {
            if let Some(mesh) = selectionMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for firstPersonMesh in &mut self.firstPersonMeshes {
            if let Some(mesh) = firstPersonMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for hudMesh in &mut self.hudMeshes {
            if let Some(mesh) = hudMesh.take() {
                destroy_buffer(device, Some(mesh.vertexBuffer));
                destroy_buffer(device, Some(mesh.indexBuffer));
            }
        }
        for retired in &mut self.retiredBuffers {
            for buffer in retired.drain(..) {
                destroy_buffer(device, Some(buffer));
            }
        }
        for staging in &mut self.stagingBuffers {
            destroy_staging_buffer(device, staging.take());
        }
        for copies in &mut self.pendingCopies {
            copies.clear();
        }
        for pending in &mut self.pendingTextureUploads {
            if let Some(pending) = pending.take() { destroy_buffer(device, Some(pending.staging)); }
        }
        for texture in &mut self.textures { destroy_texture(device, texture.take()); }
        unsafe {
            if self.pipelineLayout != vk::PipelineLayout::null() {
                device.destroy_pipeline_layout(self.pipelineLayout, None);
                self.pipelineLayout = vk::PipelineLayout::null();
            }
            if self.descriptorPool != vk::DescriptorPool::null() {
                device.destroy_descriptor_pool(self.descriptorPool, None);
                self.descriptorPool = vk::DescriptorPool::null();
            }
            if self.descriptorSetLayout != vk::DescriptorSetLayout::null() {
                device.destroy_descriptor_set_layout(self.descriptorSetLayout, None);
                self.descriptorSetLayout = vk::DescriptorSetLayout::null();
            }
        }
        self.descriptorSets.clear();
        self.uploadedAtlasRevisions.fill(0);
        self.loggedFirstChunkUpload = false;
        self.loggedFirstDraw = false;
        self.performanceLogStarted = Instant::now();
        self.performanceFrames = 0;
    }

    fn create_swapchain_resources(
        &mut self,
        device: &Device,
        memoryProperties: &vk::PhysicalDeviceMemoryProperties,
        swapchainImages: &[vk::Image],
        swapchainFormat: vk::Format,
        swapchainExtent: vk::Extent2D,
    ) -> anyhow::Result<()> {
        for &image in swapchainImages {
            self.swapchainImageViews.push(create_image_view(
                device,
                image,
                swapchainFormat,
                vk::ImageAspectFlags::COLOR,
            )?);
        }
        for _ in swapchainImages {
            let (depthImage, depthMemory) = create_image(
                device,
                memoryProperties,
                swapchainExtent.width,
                swapchainExtent.height,
                self.depthFormat,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            )?;
            let depthView = match create_image_view(
                device,
                depthImage,
                self.depthFormat,
                depth_aspect(self.depthFormat),
            ) {
                Ok(view) => view,
                Err(error) => {
                    unsafe {
                        device.destroy_image(depthImage, None);
                        device.free_memory(depthMemory, None);
                    }
                    return Err(error);
                }
            };
            self.depthImages.push(depthImage);
            self.depthMemories.push(depthMemory);
            self.depthViews.push(depthView);
        }
        self.renderPass = create_render_pass(device, swapchainFormat, self.depthFormat)?;
        let mut createdPipelines = Vec::new();
        let mut create = |blendMode,
                          depthTestEnable,
                          depthWriteEnable,
                          depthCompareOp,
                          cullMode,
                          topology,
                          lineWidth,
                          depthBias: Option<(f32, f32)>,
                          colorWriteMask| {
            let pipeline = create_pipeline(
                device,
                self.renderPass,
                self.pipelineLayout,
                blendMode,
                depthTestEnable,
                depthWriteEnable,
                depthCompareOp,
                cullMode,
                topology,
                lineWidth,
                depthBias,
                colorWriteMask,
            )?;
            createdPipelines.push(pipeline);
            Ok::<vk::Pipeline, anyhow::Error>(pipeline)
        };
        let pipelines = (|| -> anyhow::Result<[vk::Pipeline; 17]> {
            let lessEqual = vk::CompareOp::LESS_OR_EQUAL;
            let triangles = vk::PrimitiveTopology::TRIANGLE_LIST;
            let opaque = create(
                PipelineBlendMode::Disabled,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                None,
                true,
            )?;
            // `RenderPlayer.doRender` applies the PLAYER_SKIN blend profile
            // while retaining depth writes.
            let entity = create(
                PipelineBlendMode::Alpha,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            // RenderTntMinecart: texture/lighting disabled, depth retained,
            // blendFunc(SRC_ALPHA, DST_ALPHA). The block model itself keeps
            // ordinary back-face culling.
            let entityOverlay = create(
                PipelineBlendMode::TntFlash,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                None,
                true,
            )?;
            // TileEntityEndPortalRenderer switches to blendFunc(ONE, ONE)
            // from layer one onward and keeps depth test while disabling
            // depth writes through the enclosing TESR pass.
            let endPortalAdditive = create(
                PipelineBlendMode::Additive,
                true,
                false,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            // TileEntityBeaconRenderer inner beam: blend disabled, depth test
            // and writes enabled, lighting/fog disabled, culling disabled.
            let beaconCore = create(
                PipelineBlendMode::Disabled,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            // TileEntityBeaconRenderer outer glow: source-alpha blending,
            // depth test retained, depth writes and culling disabled.
            let beaconGlow = create(
                PipelineBlendMode::Alpha,
                true,
                false,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            let skyCelestial = create(
                PipelineBlendMode::SourceAlphaAdditive,
                false,
                false,
                vk::CompareOp::ALWAYS,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            // ModelBoat#renderMultipass repeats the normal entity alpha-test
            // and depth state but calls colorMask(false,false,false,false).
            let entityDepth = create(
                PipelineBlendMode::Alpha,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                false,
            )?;
            // RenderGlobal.drawBlockDamageTexture: blendFunc(DST_COLOR,
            // SRC_COLOR), depthMask(true), normal back-face culling and
            // doPolygonOffset(-3.0F, -3.0F). Vulkan's fixed depth bias is the
            // direct rasterization-state equivalent of that OpenGL polygon
            // offset for filled triangles.
            let damage = create(
                PipelineBlendMode::BlockDamage,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                Some((-3.0, -3.0)),
                true,
            )?;
            // RenderGlobal.drawSelectionBox: GL_LINE_STRIP, alpha blending,
            // depth test retained, depthMask(false), no face culling.
            let selection = create(
                PipelineBlendMode::Alpha,
                true,
                false,
                lessEqual,
                vk::CullModeFlags::NONE,
                vk::PrimitiveTopology::LINE_STRIP,
                self.selectionLineWidth,
                None,
                true,
            )?;
            // ItemRenderer.renderItemModel keeps depth test/write and back-face
            // culling after EntityRenderer clears only the hand-pass depth.
            let firstPerson = create(
                PipelineBlendMode::Alpha,
                true,
                true,
                lessEqual,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                None,
                true,
            )?;
            // ItemRenderer#renderFireInFirstPerson switches depthFunc to
            // GL_ALWAYS, disables depth writes and enables alpha blending.
            let firstPersonFire = create(
                PipelineBlendMode::Alpha,
                false,
                false,
                vk::CompareOp::ALWAYS,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                None,
                true,
            )?;
            // RenderItem.func_191966_a disables depth writes and uses GL_EQUAL
            // while drawing the two enchanted-item texture-matrix passes.
            let firstPersonGlint = create(
                PipelineBlendMode::Glint,
                true,
                false,
                vk::CompareOp::EQUAL,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                None,
                true,
            )?;
            let translucent = create(
                PipelineBlendMode::Alpha,
                true,
                false,
                lessEqual,
                vk::CullModeFlags::BACK,
                triangles,
                1.0,
                None,
                true,
            )?;
            let hud = create(
                PipelineBlendMode::Alpha,
                false,
                false,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            let crosshair = create(
                PipelineBlendMode::InvertCrosshair,
                false,
                false,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            let glint = create(
                PipelineBlendMode::Glint,
                false,
                false,
                lessEqual,
                vk::CullModeFlags::NONE,
                triangles,
                1.0,
                None,
                true,
            )?;
            Ok([
                opaque,
                entity,
                entityOverlay,
                endPortalAdditive,
                beaconCore,
                beaconGlow,
                skyCelestial,
                entityDepth,
                damage,
                selection,
                firstPerson,
                firstPersonFire,
                firstPersonGlint,
                translucent,
                hud,
                crosshair,
                glint,
            ])
        })();
        drop(create);
        let [
            opaquePipeline,
            entityPipeline,
            entityOverlayPipeline,
            endPortalAdditivePipeline,
            beaconCorePipeline,
            beaconGlowPipeline,
            skyCelestialPipeline,
            entityDepthPipeline,
            damagePipeline,
            selectionPipeline,
            firstPersonPipeline,
            firstPersonFirePipeline,
            firstPersonGlintPipeline,
            translucentPipeline,
            hudPipeline,
            crosshairPipeline,
            glintPipeline,
        ] = match pipelines {
            Ok(pipelines) => pipelines,
            Err(error) => {
                unsafe {
                    for pipeline in createdPipelines.drain(..) {
                        device.destroy_pipeline(pipeline, None);
                    }
                }
                return Err(error);
            }
        };
        self.opaquePipeline = opaquePipeline;
        self.entityPipeline = entityPipeline;
        self.entityOverlayPipeline = entityOverlayPipeline;
        self.endPortalAdditivePipeline = endPortalAdditivePipeline;
        self.beaconCorePipeline = beaconCorePipeline;
        self.beaconGlowPipeline = beaconGlowPipeline;
        self.skyCelestialPipeline = skyCelestialPipeline;
        self.entityDepthPipeline = entityDepthPipeline;
        self.damagePipeline = damagePipeline;
        self.selectionPipeline = selectionPipeline;
        self.firstPersonPipeline = firstPersonPipeline;
        self.firstPersonFirePipeline = firstPersonFirePipeline;
        self.firstPersonGlintPipeline = firstPersonGlintPipeline;
        self.translucentPipeline = translucentPipeline;
        self.hudPipeline = hudPipeline;
        self.crosshairPipeline = crosshairPipeline;
        self.glintPipeline = glintPipeline;
        for (&colorView, &depthView) in self
            .swapchainImageViews
            .iter()
            .zip(self.depthViews.iter())
        {
            let attachments = [colorView, depthView];
            let info = vk::FramebufferCreateInfo::default()
                .render_pass(self.renderPass)
                .attachments(&attachments)
                .width(swapchainExtent.width)
                .height(swapchainExtent.height)
                .layers(1);
            self.framebuffers.push(
                unsafe { device.create_framebuffer(&info, None) }
                    .context("failed creating Vulkan world framebuffer")?,
            );
        }
        Ok(())
    }
}

fn create_render_pass(
    device: &Device,
    colorFormat: vk::Format,
    depthFormat: vk::Format,
) -> anyhow::Result<vk::RenderPass> {
    let colorAttachment = vk::AttachmentDescription::default()
        .format(colorFormat)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
    let depthAttachment = vk::AttachmentDescription::default()
        .format(depthFormat)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::DONT_CARE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let attachments = [colorAttachment, depthAttachment];
    let colorReference = vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };
    let depthReference = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let colorReferences = [colorReference];
    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&colorReferences)
        .depth_stencil_attachment(&depthReference);
    let subpasses = [subpass];
    let dependency = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_stage_mask(
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );
    let dependencies = [dependency];
    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    unsafe { device.create_render_pass(&info, None) }
        .context("failed creating Vulkan world render pass")
}

fn create_pipeline(
    device: &Device,
    renderPass: vk::RenderPass,
    pipelineLayout: vk::PipelineLayout,
    blendMode: PipelineBlendMode,
    depthTestEnable: bool,
    depthWriteEnable: bool,
    depthCompareOp: vk::CompareOp,
    cullMode: vk::CullModeFlags,
    topology: vk::PrimitiveTopology,
    lineWidth: f32,
    depthBias: Option<(f32, f32)>,
    colorWriteMask: bool,
) -> anyhow::Result<vk::Pipeline> {
    let vertexCode = ash::util::read_spv(&mut Cursor::new(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/world.vert.spv"
    ))))
    .context("failed reading compiled world vertex shader")?;
    let fragmentCode = ash::util::read_spv(&mut Cursor::new(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/world.frag.spv"
    ))))
    .context("failed reading compiled world fragment shader")?;
    let vertexInfo = vk::ShaderModuleCreateInfo::default().code(&vertexCode);
    let fragmentInfo = vk::ShaderModuleCreateInfo::default().code(&fragmentCode);
    let vertexModule = unsafe { device.create_shader_module(&vertexInfo, None) }
        .context("failed creating world vertex shader module")?;
    let fragmentModule = match unsafe { device.create_shader_module(&fragmentInfo, None) } {
        Ok(module) => module,
        Err(error) => {
            unsafe { device.destroy_shader_module(vertexModule, None) };
            return Err(anyhow!("failed creating world fragment shader module: {error:?}"));
        }
    };
    let entryPoint = CString::new("main").expect("static shader entry point");
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertexModule)
            .name(&entryPoint),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragmentModule)
            .name(&entryPoint),
    ];
    let binding = vk::VertexInputBindingDescription {
        binding: 0,
        stride: WorldVertex::STRIDE,
        input_rate: vk::VertexInputRate::VERTEX,
    };
    let bindings = [binding];
    let attributes = [
        vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        },
        vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 12,
        },
        vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 20,
        },
        vk::VertexInputAttributeDescription {
            location: 3,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 36,
        },
    ];
    let vertexInput = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&attributes);
    let inputAssembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(topology)
        .primitive_restart_enable(false);
    let viewportState = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false)
        .rasterizer_discard_enable(false)
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(cullMode)
        // FaceBakery quads are counter-clockwise when viewed from outside.
        // With the current Vulkan projection/viewport pair they remain Vulkan
        // COUNTER_CLOCKWISE front faces. CLOCKWISE inverted the cull test and
        // removed exactly the block surfaces facing the camera.
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .depth_bias_enable(depthBias.is_some())
        .depth_bias_constant_factor(depthBias.map_or(0.0, |bias| bias.0))
        .depth_bias_clamp(0.0)
        .depth_bias_slope_factor(depthBias.map_or(0.0, |bias| bias.1))
        .line_width(lineWidth);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .sample_shading_enable(false);
    let depthStencil = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(depthTestEnable)
        .depth_write_enable(depthWriteEnable)
        .depth_compare_op(depthCompareOp)
        .depth_bounds_test_enable(false)
        .stencil_test_enable(false);
    let (
        blendEnable,
        srcColorBlendFactor,
        dstColorBlendFactor,
        srcAlphaBlendFactor,
        dstAlphaBlendFactor,
    ) = match blendMode {
        PipelineBlendMode::Disabled => (
            false,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
        ),
        PipelineBlendMode::Alpha => (
            true,
            vk::BlendFactor::SRC_ALPHA,
            vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
        ),
        // GlStateManager.tryBlendFuncSeparate(ONE_MINUS_DST_COLOR,
        // ONE_MINUS_SRC_COLOR, ONE, ZERO), as used by GuiIngame's
        // crosshair in Minecraft 1.12.2.
        PipelineBlendMode::InvertCrosshair => (
            true,
            vk::BlendFactor::ONE_MINUS_DST_COLOR,
            vk::BlendFactor::ONE_MINUS_SRC_COLOR,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
        ),
        // RenderItem.func_191966_a: blendFunc(SRC_COLOR, ONE).
        PipelineBlendMode::Glint => (
            true,
            vk::BlendFactor::SRC_COLOR,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
        ),
        // RenderGlobal.drawBlockDamageTexture:
        // tryBlendFuncSeparate(DST_COLOR, SRC_COLOR, ONE, ZERO).
        PipelineBlendMode::BlockDamage => (
            true,
            vk::BlendFactor::DST_COLOR,
            vk::BlendFactor::SRC_COLOR,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
        ),
        // RenderTntMinecart calls blendFunc(SRC_ALPHA, DST_ALPHA), not the
        // ordinary source-alpha/one-minus-source-alpha entity blend.
        PipelineBlendMode::TntFlash => (
            true,
            vk::BlendFactor::SRC_ALPHA,
            vk::BlendFactor::DST_ALPHA,
            vk::BlendFactor::SRC_ALPHA,
            vk::BlendFactor::DST_ALPHA,
        ),
        PipelineBlendMode::Additive => (
            true,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ONE,
        ),
        PipelineBlendMode::SourceAlphaAdditive => (
            true,
            vk::BlendFactor::SRC_ALPHA,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ZERO,
        ),
    };
    let blendAttachment = vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(blendEnable)
        .src_color_blend_factor(srcColorBlendFactor)
        .dst_color_blend_factor(dstColorBlendFactor)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(srcAlphaBlendFactor)
        .dst_alpha_blend_factor(dstAlphaBlendFactor)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(if colorWriteMask {
            vk::ColorComponentFlags::RGBA
        } else {
            vk::ColorComponentFlags::empty()
        });
    let blendAttachments = [blendAttachment];
    let colorBlend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&blendAttachments);
    let dynamicStates = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamicStates);
    let pipelineInfo = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertexInput)
        .input_assembly_state(&inputAssembly)
        .viewport_state(&viewportState)
        .rasterization_state(&rasterization)
        .multisample_state(&multisample)
        .depth_stencil_state(&depthStencil)
        .color_blend_state(&colorBlend)
        .dynamic_state(&dynamic)
        .layout(pipelineLayout)
        .render_pass(renderPass)
        .subpass(0);
    let result = unsafe {
        device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipelineInfo], None)
    };
    unsafe {
        device.destroy_shader_module(fragmentModule, None);
        device.destroy_shader_module(vertexModule, None);
    }
    result
        .map(|pipelines| pipelines[0])
        .map_err(|(_, error)| anyhow!("failed creating Vulkan world pipeline: {error:?}"))
}

fn create_host_buffer(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    usage: vk::BufferUsageFlags,
    bytes: &[u8],
) -> anyhow::Result<GpuBuffer> {
    let size = bytes.len().max(1) as vk::DeviceSize;
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&info, None) }
        .context("failed creating world GPU buffer")?;
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memoryType = find_memory_type(
        memoryProperties,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .context("no host-visible coherent memory type for world mesh")?;
    let allocationInfo = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memoryType);
    let memory = match unsafe { device.allocate_memory(&allocationInfo, None) } {
        Ok(memory) => memory,
        Err(error) => {
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(anyhow!("failed allocating world GPU buffer: {error:?}"));
        }
    };
    if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(anyhow!("failed binding world GPU buffer memory: {error:?}"));
    }
    let mapped = match unsafe { device.map_memory(memory, 0, size, vk::MemoryMapFlags::empty()) } {
        Ok(pointer) => match NonNull::new(pointer.cast::<u8>()) {
            Some(pointer) => pointer,
            None => {
                unsafe {
                    device.free_memory(memory, None);
                    device.destroy_buffer(buffer, None);
                }
                return Err(anyhow!("Vulkan returned null mapped world-buffer memory"));
            }
        },
        Err(error) => {
            unsafe {
                device.free_memory(memory, None);
                device.destroy_buffer(buffer, None);
            }
            return Err(anyhow!("failed mapping world GPU buffer: {error:?}"));
        }
    };
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.as_ptr(), bytes.len());
        device.unmap_memory(memory);
    }
    Ok(GpuBuffer { buffer, memory, size })
}

fn create_device_local_buffer(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    usage: vk::BufferUsageFlags,
    size: vk::DeviceSize,
) -> anyhow::Result<GpuBuffer> {
    let size = size.max(1);
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&info, None) }
        .context("failed creating device-local world buffer")?;
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memoryType = match find_memory_type(
        memoryProperties,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    ) {
        Some(memoryType) => memoryType,
        None => {
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(anyhow!("no device-local memory type for world mesh"));
        }
    };
    let allocationInfo = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memoryType);
    let memory = match unsafe { device.allocate_memory(&allocationInfo, None) } {
        Ok(memory) => memory,
        Err(error) => {
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(anyhow!("failed allocating device-local world buffer: {error:?}"));
        }
    };
    if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(anyhow!("failed binding device-local world buffer: {error:?}"));
    }
    Ok(GpuBuffer { buffer, memory, size })
}

fn ensure_staging_capacity(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    slot: &mut Option<FrameStagingBuffer>,
    required: vk::DeviceSize,
) -> anyhow::Result<()> {
    if slot
        .as_ref()
        .is_some_and(|staging| staging.capacity >= required.max(1))
    {
        return Ok(());
    }
    destroy_staging_buffer(device, slot.take());
    let capacity = required.max(1).next_power_of_two();
    *slot = Some(create_staging_buffer(device, memoryProperties, capacity)?);
    Ok(())
}

fn ensure_indirect_capacity(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    slot: &mut Option<FrameStagingBuffer>,
    required: vk::DeviceSize,
) -> anyhow::Result<()> {
    if slot
        .as_ref()
        .is_some_and(|buffer| buffer.capacity >= required.max(1))
    {
        return Ok(());
    }
    destroy_staging_buffer(device, slot.take());
    let capacity = required.max(1).next_power_of_two();
    *slot = Some(create_mapped_frame_buffer(
        device,
        memoryProperties,
        capacity,
        vk::BufferUsageFlags::INDIRECT_BUFFER,
        "chunk indirect",
    )?);
    Ok(())
}

fn create_staging_buffer(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    capacity: vk::DeviceSize,
) -> anyhow::Result<FrameStagingBuffer> {
    create_mapped_frame_buffer(
        device,
        memoryProperties,
        capacity,
        vk::BufferUsageFlags::TRANSFER_SRC,
        "world staging",
    )
}

fn create_mapped_frame_buffer(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    capacity: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    label: &str,
) -> anyhow::Result<FrameStagingBuffer> {
    let info = vk::BufferCreateInfo::default()
        .size(capacity)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&info, None) }
        .with_context(|| format!("failed creating {label} buffer"))?;
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memoryType = match find_memory_type(
        memoryProperties,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    ) {
        Some(memoryType) => memoryType,
        None => {
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(anyhow!("no host-visible coherent memory type for {label}"));
        }
    };
    let allocationInfo = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memoryType);
    let memory = match unsafe { device.allocate_memory(&allocationInfo, None) } {
        Ok(memory) => memory,
        Err(error) => {
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(anyhow!("failed allocating {label} memory: {error:?}"));
        }
    };
    if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(anyhow!("failed binding {label} memory: {error:?}"));
    }
    let mapped = match unsafe {
        device.map_memory(memory, 0, capacity, vk::MemoryMapFlags::empty())
    } {
        Ok(pointer) => match NonNull::new(pointer.cast::<u8>()) {
            Some(pointer) => pointer,
            None => {
                unsafe {
                    device.free_memory(memory, None);
                    device.destroy_buffer(buffer, None);
                }
                return Err(anyhow!("Vulkan returned null mapped {label} memory"));
            }
        },
        Err(error) => {
            unsafe {
                device.free_memory(memory, None);
                device.destroy_buffer(buffer, None);
            }
            return Err(anyhow!("failed mapping {label} memory: {error:?}"));
        }
    };
    Ok(FrameStagingBuffer { buffer, memory, mapped, capacity })
}

fn destroy_staging_buffer(device: &Device, staging: Option<FrameStagingBuffer>) {
    if let Some(staging) = staging {
        unsafe {
            device.unmap_memory(staging.memory);
            device.destroy_buffer(staging.buffer, None);
            device.free_memory(staging.memory, None);
        }
    }
}

fn align_up(value: u64, alignment: u64) -> u64 {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn create_pending_texture_upload(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> anyhow::Result<(GpuTexture, PendingTextureUpload)> {
    anyhow::ensure!(
        rgba.len() == width as usize * height as usize * 4,
        "block atlas byte count does not match its dimensions",
    );
    let staging = create_host_buffer(
        device,
        memoryProperties,
        vk::BufferUsageFlags::TRANSFER_SRC,
        rgba,
    )?;
    let format = vk::Format::R8G8B8A8_SRGB;
    let (image, memory) = match create_image(
        device,
        memoryProperties,
        width,
        height,
        format,
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
    ) {
        Ok(result) => result,
        Err(error) => {
            destroy_buffer(device, Some(staging));
            return Err(error);
        }
    };
    let view = match create_image_view(device, image, format, vk::ImageAspectFlags::COLOR) {
        Ok(view) => view,
        Err(error) => {
            destroy_buffer(device, Some(staging));
            unsafe { device.destroy_image(image, None); device.free_memory(memory, None); }
            return Err(error);
        }
    };
    let samplerInfo = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::NEAREST)
        .min_filter(vk::Filter::NEAREST)
        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .mip_lod_bias(0.0)
        .anisotropy_enable(false)
        .max_anisotropy(1.0)
        .compare_enable(false)
        .min_lod(0.0)
        .max_lod(0.0)
        .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
        .unnormalized_coordinates(false);
    let sampler = match unsafe { device.create_sampler(&samplerInfo, None) } {
        Ok(sampler) => sampler,
        Err(error) => {
            destroy_buffer(device, Some(staging));
            unsafe {
                device.destroy_image_view(view, None);
                device.destroy_image(image, None);
                device.free_memory(memory, None);
            }
            return Err(anyhow!("failed creating block-atlas sampler: {error:?}"));
        }
    };
    Ok((
        GpuTexture { image, memory, view, sampler },
        PendingTextureUpload { staging, image, width, height },
    ))
}

fn create_image(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    width: u32,
    height: u32,
    format: vk::Format,
    usage: vk::ImageUsageFlags,
) -> anyhow::Result<(vk::Image, vk::DeviceMemory)> {
    let info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = unsafe { device.create_image(&info, None) }
        .context("failed creating Vulkan world image")?;
    let requirements = unsafe { device.get_image_memory_requirements(image) };
    let memoryType = find_memory_type(
        memoryProperties,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .context("no device-local memory type for Vulkan world image")?;
    let allocationInfo = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memoryType);
    let memory = match unsafe { device.allocate_memory(&allocationInfo, None) } {
        Ok(memory) => memory,
        Err(error) => {
            unsafe { device.destroy_image(image, None) };
            return Err(anyhow!("failed allocating Vulkan world image memory: {error:?}"));
        }
    };
    if let Err(error) = unsafe { device.bind_image_memory(image, memory, 0) } {
        unsafe {
            device.free_memory(memory, None);
            device.destroy_image(image, None);
        }
        return Err(anyhow!("failed binding Vulkan world image memory: {error:?}"));
    }
    Ok((image, memory))
}

fn create_image_view(
    device: &Device,
    image: vk::Image,
    format: vk::Format,
    aspect: vk::ImageAspectFlags,
) -> anyhow::Result<vk::ImageView> {
    let info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(aspect)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
        );
    unsafe { device.create_image_view(&info, None) }
        .context("failed creating Vulkan world image view")
}

fn destroy_buffer(device: &Device, buffer: Option<GpuBuffer>) {
    if let Some(buffer) = buffer {
        let _ = buffer.size;
        unsafe {
            if buffer.buffer != vk::Buffer::null() {
                device.destroy_buffer(buffer.buffer, None);
            }
            if buffer.memory != vk::DeviceMemory::null() {
                device.free_memory(buffer.memory, None);
            }
        }
    }
}

fn destroy_texture(device: &Device, texture: Option<GpuTexture>) {
    if let Some(texture) = texture {
        unsafe {
            device.destroy_sampler(texture.sampler, None);
            device.destroy_image_view(texture.view, None);
            device.destroy_image(texture.image, None);
            device.free_memory(texture.memory, None);
        }
    }
}

fn find_depth_format(
    instance: &Instance,
    physicalDevice: vk::PhysicalDevice,
) -> Option<vk::Format> {
    [
        vk::Format::D32_SFLOAT,
        vk::Format::D24_UNORM_S8_UINT,
        vk::Format::D16_UNORM,
    ]
    .into_iter()
    .find(|&format| {
        let properties = unsafe { instance.get_physical_device_format_properties(physicalDevice, format) };
        properties
            .optimal_tiling_features
            .contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)
    })
}

fn find_memory_type(
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

fn color_subresource() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
}

fn depth_aspect(format: vk::Format) -> vk::ImageAspectFlags {
    match format {
        vk::Format::D24_UNORM_S8_UINT | vk::Format::D32_SFLOAT_S8_UINT => {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        }
        _ => vk::ImageAspectFlags::DEPTH,
    }
}


fn dynamic_mesh_needs_upload<T>(
    slot: &Option<GpuEntityMesh>,
    vertices: &[T],
    indices: &[u32],
    contentGeneration: u64,
) -> bool {
    if vertices.is_empty() || indices.is_empty() {
        return false;
    }
    let vertexBytes = std::mem::size_of_val(vertices) as u64;
    let indexBytes = std::mem::size_of_val(indices) as u64;
    let Ok(indexCount) = u32::try_from(indices.len()) else {
        return true;
    };
    !slot.as_ref().is_some_and(|mesh| {
        mesh.contentGeneration == Some(contentGeneration)
            && mesh.indexCount == indexCount
            && mesh.vertexBuffer.size >= vertexBytes
            && mesh.indexBuffer.size >= indexBytes
    })
}

fn append_dynamic_upload_bytes<T>(
    total: u64,
    slot: &Option<GpuEntityMesh>,
    vertices: &[T],
    indices: &[u32],
    contentGeneration: u64,
) -> u64 {
    if !dynamic_mesh_needs_upload(slot, vertices, indices, contentGeneration) {
        return total;
    }
    let vertexBytes = std::mem::size_of_val(vertices) as u64;
    let indexBytes = std::mem::size_of_val(indices) as u64;
    let afterVertices = align_up(total, 4).saturating_add(vertexBytes);
    align_up(afterVertices, 4).saturating_add(indexBytes)
}

fn clear_empty_dynamic_mesh<T>(
    slot: &mut Option<GpuEntityMesh>,
    vertices: &[T],
    indices: &[u32],
) {
    if vertices.is_empty() || indices.is_empty() {
        clear_dynamic_mesh(slot);
    }
}

fn clear_dynamic_mesh(slot: &mut Option<GpuEntityMesh>) {
    if let Some(mesh) = slot.as_mut() {
        mesh.indexCount = 0;
        mesh.contentGeneration = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn stage_dynamic_mesh<T>(
    device: &Device,
    memoryProperties: &vk::PhysicalDeviceMemoryProperties,
    slot: &mut Option<GpuEntityMesh>,
    staging: &FrameStagingBuffer,
    pendingCopies: &mut Vec<PendingBufferCopy>,
    stagingOffset: &mut u64,
    vertices: &[T],
    indices: &[u32],
    contentGeneration: u64,
    label: &str,
) -> anyhow::Result<()> {
    if vertices.is_empty() || indices.is_empty() {
        clear_dynamic_mesh(slot);
        return Ok(());
    }

    let vertexBytes = as_bytes(vertices);
    let indexBytes = as_bytes(indices);
    let indexCount = u32::try_from(indices.len())
        .with_context(|| format!("{label} index count exceeds u32"))?;
    if slot.as_ref().is_some_and(|mesh| {
        mesh.contentGeneration == Some(contentGeneration)
            && mesh.indexCount == indexCount
            && mesh.vertexBuffer.size >= vertexBytes.len() as u64
            && mesh.indexBuffer.size >= indexBytes.len() as u64
    }) {
        return Ok(());
    }

    let needsReplacement = slot.as_ref().map_or(true, |mesh| {
        mesh.vertexBuffer.size < vertexBytes.len() as u64
            || mesh.indexBuffer.size < indexBytes.len() as u64
    });
    if needsReplacement {
        if let Some(old) = slot.take() {
            // The owner waits for this frame slot's fence before upload.
            destroy_buffer(device, Some(old.vertexBuffer));
            destroy_buffer(device, Some(old.indexBuffer));
        }
        let vertexCapacity = (vertexBytes.len() as u64).next_power_of_two().max(256);
        let indexCapacity = (indexBytes.len() as u64).next_power_of_two().max(256);
        let vertexBuffer = create_device_local_buffer(
            device,
            memoryProperties,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            vertexCapacity,
        )?;
        let indexBuffer = match create_device_local_buffer(
            device,
            memoryProperties,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            indexCapacity,
        ) {
            Ok(buffer) => buffer,
            Err(error) => {
                destroy_buffer(device, Some(vertexBuffer));
                return Err(error);
            }
        };
        *slot = Some(GpuEntityMesh {
            vertexBuffer,
            indexBuffer,
            indexCount: 0,
            contentGeneration: None,
        });
    }

    let mesh = slot
        .as_mut()
        .with_context(|| format!("{label} mesh allocation missing after resize"))?;
    let vertexOffset = align_up(*stagingOffset, 4);
    let indexOffset = align_up(vertexOffset + vertexBytes.len() as u64, 4);
    let endOffset = indexOffset + indexBytes.len() as u64;
    anyhow::ensure!(
        endOffset <= staging.capacity,
        "{label} mesh exceeded world staging capacity"
    );
    unsafe {
        std::ptr::copy_nonoverlapping(
            vertexBytes.as_ptr(),
            staging.mapped.as_ptr().add(vertexOffset as usize),
            vertexBytes.len(),
        );
        std::ptr::copy_nonoverlapping(
            indexBytes.as_ptr(),
            staging.mapped.as_ptr().add(indexOffset as usize),
            indexBytes.len(),
        );
    }
    pendingCopies.push(PendingBufferCopy {
        destination: mesh.vertexBuffer.buffer,
        region: vk::BufferCopy {
            src_offset: vertexOffset,
            dst_offset: 0,
            size: vertexBytes.len() as u64,
        },
        destinationAccess: vk::AccessFlags::VERTEX_ATTRIBUTE_READ,
    });
    pendingCopies.push(PendingBufferCopy {
        destination: mesh.indexBuffer.buffer,
        region: vk::BufferCopy {
            src_offset: indexOffset,
            dst_offset: 0,
            size: indexBytes.len() as u64,
        },
        destinationAccess: vk::AccessFlags::INDEX_READ,
    });
    mesh.indexCount = indexCount;
    mesh.contentGeneration = Some(contentGeneration);
    *stagingOffset = endOffset;
    Ok(())
}

#[cfg(test)]
mod chunk_batch_tests {
    use super::{ElementArenaAllocator, ChunkIndirectCommand};

    #[test]
    fn fragmented_spans_coalesce_without_reordering_live_meshes() {
        let mut allocator = ElementArenaAllocator::new(128);
        let small = allocator.claim(16).expect("small span");
        let middle = allocator.claim(48).expect("middle span");
        let tail = allocator.claim(64).expect("tail span");
        assert_eq!((small, middle, tail), (0, 16, 64));
        assert!(allocator.claim(1).is_none());

        allocator.recycle(middle, 48);
        assert_eq!(allocator.claim(32), Some(16));
        allocator.recycle(48, 16);
        allocator.recycle(small, 16);
        allocator.recycle(tail, 64);
        assert_eq!(allocator.free_count(), 128);
        assert_eq!(allocator.claim(128), Some(0));
    }

    #[test]
    fn indirect_command_matches_vulkan_layout() {
        assert_eq!(std::mem::size_of::<ChunkIndirectCommand>(), 20);
    }
}

fn as_bytes<T>(values: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            values.as_ptr().cast::<u8>(),
            std::mem::size_of_val(values),
        )
    }
}

fn struct_as_bytes<T>(value: &T) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            (value as *const T).cast::<u8>(),
            std::mem::size_of::<T>(),
        )
    }
}
