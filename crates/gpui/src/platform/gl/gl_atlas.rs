use crate::{AtlasTextureId, AtlasTile, PlatformAtlas, TileId};

pub struct GlAtlas {}
impl PlatformAtlas for GlAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &crate::AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(crate::Size<crate::DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<crate::AtlasTile>> {
        println!("TODO: get or insert in atlas");
        Ok(Some(AtlasTile {
            texture_id: AtlasTextureId {
                index: 0,
                kind: crate::AtlasTextureKind::Monochrome,
            },
            tile_id: TileId(0),
            padding: 0,
            bounds: Default::default(),
        }))
    }

    fn remove(&self, key: &crate::AtlasKey) {
        println!("TODO: remove key from atlas");
    }
}
