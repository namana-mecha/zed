use std::borrow::Cow;

use collections::FxHashMap;
use parking_lot::Mutex;

use crate::{AtlasKey, AtlasTextureId, AtlasTile, Bounds, PlatformAtlas, Point, TileId};

pub struct GlAtlasState {
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    data_by_key: FxHashMap<AtlasKey, Vec<u8>>,
    next_index: u32,
}

pub struct GlAtlas(Mutex<GlAtlasState>);
impl GlAtlas {
    pub fn new() -> Self {
        Self(Mutex::new(GlAtlasState {
            tiles_by_key: Default::default(),
            data_by_key: Default::default(),
            next_index: Default::default(),
        }))
    }
}

impl PlatformAtlas for GlAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &crate::AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(crate::Size<crate::DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<crate::AtlasTile>> {
        let mut lock = self.0.lock();

        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(tile.clone()))
        } else {
            let Some((size, bytes)) = build()? else {
                return Ok(None);
            };

            let tile = AtlasTile {
                texture_id: AtlasTextureId {
                    index: lock.next_index,
                    kind: key.texture_kind(),
                },
                tile_id: TileId(lock.next_index),
                padding: 0,
                bounds: Bounds::new(Default::default(), size),
            };

            lock.data_by_key
                .entry(key.clone())
                .insert_entry(bytes.into());

            lock.next_index += 1;

            Ok(Some(
                lock.tiles_by_key
                    .entry(key.clone())
                    .insert_entry(tile)
                    .get()
                    .clone(),
            ))
        }
    }

    fn remove(&self, key: &crate::AtlasKey) {
        let mut lock = self.0.lock();

        if let Some(_) = lock.tiles_by_key.remove(key) {
            lock.data_by_key.remove(key);
        }
    }
}
