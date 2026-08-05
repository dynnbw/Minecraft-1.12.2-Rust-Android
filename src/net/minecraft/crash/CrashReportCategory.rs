use crate::net::minecraft::util::math::BlockPos::BlockPos;

/// Rust equivalent of MCP `net.minecraft.crash.CrashReportCategory`.
///
/// Categories retain ordered key/value details so the final crash report has
/// the same section-oriented structure as the 1.12.2 client.
#[derive(Clone, Debug)]
pub struct CrashReportCategory {
    pub name: String,
    children: Vec<Entry>,
}

#[derive(Clone, Debug)]
struct Entry {
    key: String,
    value: String,
}

impl CrashReportCategory {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            children: Vec::new(),
        }
    }

    pub fn setDetail<F, E>(&mut self, name: impl Into<String>, detail: F)
    where
        F: FnOnce() -> Result<String, E>,
        E: std::fmt::Display,
    {
        let value = match detail() {
            Ok(value) => value,
            Err(error) => format!("~~ERROR~~ {error}"),
        };
        self.addCrashSection(name, value);
    }

    pub fn addCrashSection(&mut self, sectionName: impl Into<String>, value: impl ToString) {
        self.children.push(Entry {
            key: sectionName.into(),
            value: value.to_string(),
        });
    }

    pub fn addCrashSectionThrowable(
        &mut self,
        sectionName: impl Into<String>,
        throwable: impl std::fmt::Display,
    ) {
        self.addCrashSection(sectionName, format!("~~ERROR~~ {throwable}"));
    }

    pub fn appendToStringBuilder(&self, builder: &mut String) {
        builder.push_str("-- ");
        builder.push_str(&self.name);
        builder.push_str(" --\nDetails:");
        for entry in &self.children {
            builder.push_str("\n\t");
            builder.push_str(&entry.key);
            builder.push_str(": ");
            builder.push_str(&entry.value);
        }
    }

    pub fn getCoordinateInfo(pos: BlockPos) -> String {
        Self::getCoordinateInfoXYZ(pos.x, pos.y, pos.z)
    }

    pub fn getCoordinateInfoXYZ(x: i32, y: i32, z: i32) -> String {
        let chunkX = x >> 4;
        let chunkZ = z >> 4;
        let localX = x & 15;
        let sectionY = y >> 4;
        let localZ = z & 15;
        let chunkMinX = chunkX << 4;
        let chunkMinZ = chunkZ << 4;
        let chunkMaxX = ((chunkX + 1) << 4) - 1;
        let chunkMaxZ = ((chunkZ + 1) << 4) - 1;
        let regionX = x >> 9;
        let regionZ = z >> 9;
        let regionMinChunkX = regionX << 5;
        let regionMinChunkZ = regionZ << 5;
        let regionMaxChunkX = ((regionX + 1) << 5) - 1;
        let regionMaxChunkZ = ((regionZ + 1) << 5) - 1;
        let regionMinBlockX = regionX << 9;
        let regionMinBlockZ = regionZ << 9;
        let regionMaxBlockX = ((regionX + 1) << 9) - 1;
        let regionMaxBlockZ = ((regionZ + 1) << 9) - 1;
        format!(
            "World: ({x},{y},{z}), Chunk: (at {localX},{sectionY},{localZ} in {chunkX},{chunkZ}; contains blocks {chunkMinX},0,{chunkMinZ} to {chunkMaxX},255,{chunkMaxZ}), Region: ({regionX},{regionZ}; contains chunks {regionMinChunkX},{regionMinChunkZ} to {regionMaxChunkX},{regionMaxChunkZ}, blocks {regionMinBlockX},0,{regionMinBlockZ} to {regionMaxBlockX},255,{regionMaxBlockZ})"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::CrashReportCategory;

    #[test]
    fn coordinate_info_matches_mcp_chunk_and_region_layout() {
        let value = CrashReportCategory::getCoordinateInfoXYZ(31, 64, -1);
        assert!(value.contains("Chunk: (at 15,4,15 in 1,-1"));
        assert!(value.contains("Region: (0,-1"));
    }
}
