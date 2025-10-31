use collections::FxHashMap;

use crate::{AtlasKey, AtlasTextureId, AtlasTile, PlatformAtlas, TileId};

pub struct ImpellerAtlas {
    tiles_by_key: parking_lot::Mutex<FxHashMap<AtlasKey, AtlasTile>>,
}
impl ImpellerAtlas {
    pub fn new() -> Self {
        Self {
            tiles_by_key: Default::default(),
        }
    }
}
impl PlatformAtlas for ImpellerAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &crate::AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(crate::Size<crate::DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<crate::AtlasTile>> {
        let mut lock = self.tiles_by_key.lock();
        if let Some(tile) = lock.get(key) {
            Ok(Some(tile.clone()))
        } else {
            let tile = AtlasTile {
                texture_id: AtlasTextureId {
                    index: 0,
                    kind: crate::AtlasTextureKind::Monochrome,
                },
                tile_id: TileId(0),
                padding: 0,
                bounds: Default::default(),
            };
            lock.insert(key.clone(), tile.clone());
            Ok(Some(tile))
        }
    }

    fn remove(&self, key: &crate::AtlasKey) {
        todo!()
    }
}
