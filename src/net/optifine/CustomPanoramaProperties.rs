use std::collections::HashMap;
use std::fmt;

use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// Direct data equivalent of OptiFine C6 `CustomPanoramaProperties`.
/// Field and accessor names mirror the supplied MCP/OptiFine source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomPanoramaProperties {
    path: String,
    panoramaLocations: [ResourceLocation; 6],
    weight: i32,
    blur1: i32,
    blur2: i32,
    blur3: i32,
    overlay1Top: i32,
    overlay1Bottom: i32,
    overlay2Top: i32,
    overlay2Bottom: i32,
}

impl CustomPanoramaProperties {
    pub fn new(path: impl Into<String>, properties: &HashMap<String, String>) -> Self {
        let path = path.into();
        Self {
            panoramaLocations: std::array::from_fn(|index| {
                ResourceLocation::parse(format!("{path}/panorama_{index}.png"))
            }),
            path,
            weight: parseInt(properties.get("weight"), 1),
            blur1: parseInt(properties.get("blur1"), 64),
            blur2: parseInt(properties.get("blur2"), 3),
            blur3: parseInt(properties.get("blur3"), 3),
            overlay1Top: parseColor4(properties.get("overlay1.top"), -2_130_706_433),
            overlay1Bottom: parseColor4(properties.get("overlay1.bottom"), 16_777_215),
            overlay2Top: parseColor4(properties.get("overlay2.top"), 0),
            overlay2Bottom: parseColor4(properties.get("overlay2.bottom"), i32::MIN),
        }
    }

    pub fn getPanoramaLocations(&self) -> &[ResourceLocation; 6] {
        &self.panoramaLocations
    }

    pub const fn getWeight(&self) -> i32 {
        self.weight
    }

    pub const fn getBlur1(&self) -> i32 {
        self.blur1
    }

    pub const fn getBlur2(&self) -> i32 {
        self.blur2
    }

    pub const fn getBlur3(&self) -> i32 {
        self.blur3
    }

    pub const fn getOverlay1Top(&self) -> i32 {
        self.overlay1Top
    }

    pub const fn getOverlay1Bottom(&self) -> i32 {
        self.overlay1Bottom
    }

    pub const fn getOverlay2Top(&self) -> i32 {
        self.overlay2Top
    }

    pub const fn getOverlay2Bottom(&self) -> i32 {
        self.overlay2Bottom
    }
}

impl fmt::Display for CustomPanoramaProperties {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}, weight: {}, blur: {} {} {}, overlay: {} {} {} {}",
            self.path,
            self.weight,
            self.blur1,
            self.blur2,
            self.blur3,
            self.overlay1Top,
            self.overlay1Bottom,
            self.overlay2Top,
            self.overlay2Bottom
        )
    }
}

fn parseInt(value: Option<&String>, default: i32) -> i32 {
    value
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn parseColor4(value: Option<&String>, default: i32) -> i32 {
    let Some(value) = value else {
        return default;
    };
    u32::from_str_radix(value.trim(), 16)
        .map(|color| color as i32)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_defaults_match_optifine_c6() {
        let properties =
            CustomPanoramaProperties::new("textures/gui/title/background", &HashMap::new());
        assert_eq!(properties.getWeight(), 1);
        assert_eq!(properties.getBlur1(), 64);
        assert_eq!(properties.getBlur2(), 3);
        assert_eq!(properties.getBlur3(), 3);
        assert_eq!(properties.getOverlay1Top(), -2_130_706_433);
        assert_eq!(properties.getOverlay1Bottom(), 16_777_215);
        assert_eq!(properties.getOverlay2Bottom(), i32::MIN);
    }

    #[test]
    fn argb_hex_uses_java_narrowing() {
        let mut values = HashMap::new();
        values.insert("overlay1.top".to_owned(), "80FFFFFF".to_owned());
        let properties = CustomPanoramaProperties::new("test", &values);
        assert_eq!(properties.getOverlay1Top(), 0x80FF_FFFF_u32 as i32);
    }

    #[test]
    fn to_string_matches_optifine_field_order() {
        let properties =
            CustomPanoramaProperties::new("textures/gui/title/background", &HashMap::new());
        assert_eq!(
            properties.to_string(),
            "textures/gui/title/background, weight: 1, blur: 64 3 3, overlay: -2130706433 16777215 0 -2147483648"
        );
    }
}
