#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldType { worldType: String }
impl WorldType {
    pub fn parseWorldType(value: &str) -> Self {
        let worldType = match value.to_ascii_lowercase().as_str() {
            "flat" | "largebiomes" | "amplified" | "customized" | "debug_all_block_states" | "default_1_1" => value.to_ascii_lowercase(),
            _ => "default".to_owned(),
        };
        Self { worldType }
    }
    pub fn getWorldTypeName(&self) -> &str { &self.worldType }
}
