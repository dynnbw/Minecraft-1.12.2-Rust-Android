use crate::net::minecraft::client::audio::PositionedSoundRecord::{
    AttenuationType, PositionedSoundRecord,
};
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;
use crate::net::minecraft::util::SoundCategory::SoundCategory;

/// A client-originated `WorldClient#playSound` request.
///
/// Network sounds remain `PlayHandlerEvent::Sound`; this type carries sounds
/// whose source methods execute directly on the remote client, such as local
/// item use, block placement and entity-status feedback.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalSoundEvent {
    pub sound: ResourceLocation,
    pub category: SoundCategory,
    pub position: [f32; 3],
    pub volume: f32,
    pub pitch: f32,
    pub attenuationType: AttenuationType,
}

impl LocalSoundEvent {
    pub fn positioned(
        sound: impl AsRef<str>,
        category: SoundCategory,
        position: [f32; 3],
        volume: f32,
        pitch: f32,
    ) -> Self {
        Self {
            sound: ResourceLocation::parse(sound),
            category,
            position,
            volume,
            pitch,
            attenuationType: AttenuationType::Linear,
        }
    }

    pub fn intoRecord(self) -> PositionedSoundRecord {
        PositionedSoundRecord::new(
            self.sound,
            self.category,
            self.volume,
            self.pitch,
            false,
            0,
            self.attenuationType,
            self.position,
        )
    }
}
