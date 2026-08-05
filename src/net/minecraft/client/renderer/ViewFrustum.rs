use crate::net::minecraft::client::renderer::chunk::RenderChunk::RenderChunkKey;

/// Horizontal render-distance bounds used by MCP 1.12.2 `ViewFrustum`.
pub const fn containsRenderChunk(
    center: RenderChunkKey,
    candidate: RenderChunkKey,
    renderDistanceChunks: i32,
) -> bool {
    candidate.isValidWorldHeight()
        && (candidate.x - center.x).abs() <= renderDistanceChunks
        && (candidate.z - center.z).abs() <= renderDistanceChunks
}
