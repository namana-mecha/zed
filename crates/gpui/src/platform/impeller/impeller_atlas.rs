use collections::FxHashMap;
use std::sync::Arc;

use crate::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, PlatformAtlas,
    Point, Size, TileId,
};

struct SyncContext(impellers::Context);
unsafe impl Send for SyncContext {}
unsafe impl Sync for SyncContext {}

pub struct ImpellerAtlas {
    state: parking_lot::Mutex<ImpellerAtlasState>,
    context: Arc<parking_lot::Mutex<Option<SyncContext>>>,
}

struct PendingTextureCreation {
    id: AtlasTextureId,
    size: Size<DevicePixels>,
    kind: AtlasTextureKind,
    data: Vec<u8>,
}

struct ImpellerAtlasState {
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    textures: FxHashMap<AtlasTextureId, ImpellerTexture>,
    next_texture_id: u32,
    next_tile_id: u32,
    pending_creations: Vec<PendingTextureCreation>,
}

struct ImpellerTexture {
    texture: Option<impellers::Texture>,
    size: Size<DevicePixels>,
    kind: AtlasTextureKind,
}

impl ImpellerAtlas {
    pub fn new() -> Self {
        Self {
            state: parking_lot::Mutex::new(ImpellerAtlasState {
                tiles_by_key: Default::default(),
                textures: Default::default(),
                next_texture_id: 0,
                next_tile_id: 0,
                pending_creations: Vec::new(),
            }),
            context: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    pub fn set_context(&self, context: impellers::Context) {
        *self.context.lock() = Some(SyncContext(context));
    }

    pub fn before_frame(&self, context: &impellers::Context) {
        let mut state = self.state.lock();
        state.flush(context);
    }

    pub fn get_texture(&self, texture_id: AtlasTextureId) -> Option<impellers::Texture> {
        let state = self.state.lock();
        state
            .textures
            .get(&texture_id)
            .and_then(|t| t.texture.clone())
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
        let mut state = self.state.lock();

        if let Some(tile) = state.tiles_by_key.get(key) {
            return Ok(Some(tile.clone()));
        }

        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };

        let texture_kind = key.texture_kind();

        let texture_id = AtlasTextureId {
            index: state.next_texture_id,
            kind: texture_kind,
        };
        state.next_texture_id += 1;

        let tile_id = TileId(state.next_tile_id);
        state.next_tile_id += 1;

        state.pending_creations.push(PendingTextureCreation {
            id: texture_id,
            size,
            kind: texture_kind,
            data: bytes.into_owned(),
        });

        state.textures.insert(
            texture_id,
            ImpellerTexture {
                texture: None,
                size,
                kind: texture_kind,
            },
        );

        let tile = AtlasTile {
            texture_id,
            tile_id,
            padding: 0,
            bounds: Bounds {
                origin: Point {
                    x: DevicePixels(0),
                    y: DevicePixels(0),
                },
                size,
            },
        };

        state.tiles_by_key.insert(key.clone(), tile.clone());
        Ok(Some(tile))
    }

    fn remove(&self, key: &crate::AtlasKey) {
        let mut state = self.state.lock();
        if let Some(tile) = state.tiles_by_key.remove(key) {
            state.textures.remove(&tile.texture_id);
        }
    }
}

impl ImpellerAtlasState {
    fn flush(&mut self, context: &impellers::Context) {
        for creation in self.pending_creations.drain(..) {
            let width = creation.size.width.0 as u32;
            let height = creation.size.height.0 as u32;

            let gpu_texture = match creation.kind {
                AtlasTextureKind::Monochrome => {
                    let expected_size = (width * height) as usize;

                    if creation.data.len() != expected_size {
                        eprintln!(
                            "Monochrome texture size mismatch: got {} bytes, expected {} ({}x{})",
                            creation.data.len(),
                            expected_size,
                            width,
                            height
                        );
                        None
                    } else {
                        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                        for &mask in creation.data.iter() {
                            rgba_data.push(255);
                            rgba_data.push(255);
                            rgba_data.push(255);
                            rgba_data.push(mask);
                        }

                        unsafe {
                            match context.create_texture_with_rgba8(&rgba_data, width, height) {
                                Ok(texture) => Some(texture),
                                Err(e) => {
                                    eprintln!("Failed to create monochrome GPU texture: {}", e);
                                    None
                                }
                            }
                        }
                    }
                }
                AtlasTextureKind::Polychrome => {
                    let expected_size = (width * height * 4) as usize;

                    if creation.data.len() != expected_size {
                        eprintln!(
                            "Polychrome texture size mismatch: got {} bytes, expected {} ({}x{})",
                            creation.data.len(),
                            expected_size,
                            width,
                            height
                        );
                        None
                    } else {
                        let mut rgba_data = Vec::with_capacity(creation.data.len());
                        for chunk in creation.data.chunks_exact(4) {
                            let alpha = chunk[3] as f32 / 255.0;
                            rgba_data.push((chunk[2] as f32 * alpha) as u8);
                            rgba_data.push((chunk[1] as f32 * alpha) as u8);
                            rgba_data.push((chunk[0] as f32 * alpha) as u8);
                            rgba_data.push(chunk[3]);
                        }

                        unsafe {
                            match context.create_texture_with_rgba8(&rgba_data, width, height) {
                                Ok(texture) => Some(texture),
                                Err(e) => {
                                    eprintln!("Failed to create polychrome GPU texture: {}", e);
                                    None
                                }
                            }
                        }
                    }
                }
            };

            if let Some(texture_entry) = self.textures.get_mut(&creation.id) {
                texture_entry.texture = gpu_texture;
            }
        }
    }
}
