/// Client-visible subset of MCP 1.12.2 `FoodStats` updated by
/// `SPacketUpdateHealth`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoodStats {
    foodLevel: i32,
    foodSaturationLevel: f32,
}

impl Default for FoodStats {
    fn default() -> Self { Self { foodLevel: 20, foodSaturationLevel: 5.0 } }
}

impl FoodStats {
    pub const fn getFoodLevel(&self) -> i32 { self.foodLevel }
    pub const fn getSaturationLevel(&self) -> f32 { self.foodSaturationLevel }
    pub fn setFoodLevel(&mut self, value: i32) { self.foodLevel = value; }
    pub fn setFoodSaturationLevel(&mut self, value: f32) { self.foodSaturationLevel = value; }
}
