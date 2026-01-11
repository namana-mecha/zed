use crate::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, PlatformAtlas,
    Point, Size, platform::AtlasTextureList,
};
use anyhow::Result;
use collections::FxHashMap;
use etagere::BucketedAtlasAllocator;
use glow::HasContext;
use parking_lot::Mutex;
use std::{borrow::Cow, ops};

pub(crate) struct GlAtlas(Mutex<GlAtlasState>);

struct PendingTextureCreation {
    id: AtlasTextureId,
    size: Size<DevicePixels>,
    kind: AtlasTextureKind,
}

struct PendingUpload {
    id: AtlasTextureId,
    bounds: Bounds<DevicePixels>,
    data: Vec<u8>,
}

struct PendingDeletion {
    texture: glow::Texture,
}

struct GlAtlasState {
    storage: GlAtlasStorage,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    pending_creations: Vec<PendingTextureCreation>,
    pending_uploads: Vec<PendingUpload>,
    pending_deletions: Vec<PendingDeletion>,
}

// We assume the glow::Texture handles (usually u32) are Send.
unsafe impl Send for GlAtlasState {}

#[derive(Clone, Copy)]
pub struct GlTextureInfo {
    pub texture: glow::Texture,
    pub format: GlTextureFormat,
    // FIX: Add size field to TextureInfo so renderer knows correct UV scale
    pub size: Size<DevicePixels>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GlTextureFormat {
    Alpha, // Replaces R8 for OpenGL 2.1
    Rgba,  // Replaces RGBA8
}

impl GlAtlas {
    pub fn new() -> Self {
        GlAtlas(Mutex::new(GlAtlasState {
            storage: GlAtlasStorage::default(),
            tiles_by_key: FxHashMap::default(),
            pending_creations: Vec::new(),
            pending_uploads: Vec::new(),
            pending_deletions: Vec::new(),
        }))
    }

    pub fn before_frame(&self, gl: &glow::Context) {
        let mut lock = self.0.lock();
        lock.flush(gl);
    }

    pub fn get_texture_info(&self, id: AtlasTextureId) -> Option<GlTextureInfo> {
        let lock = self.0.lock();
        let texture = &lock.storage[id];
        texture.gl_texture.map(|t| GlTextureInfo {
            texture: t,
            format: texture.format,
            size: texture.size,
        })
    }
}

impl PlatformAtlas for GlAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        {
            let lock = self.0.lock();
            if let Some(tile) = lock.tiles_by_key.get(key) {
                return Ok(Some(tile.clone()));
            }
        }

        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            return Ok(Some(tile.clone()));
        }

        profiling::scope!("new tile");
        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };

        let tile = lock.allocate(size, key.texture_kind());
        lock.upload_texture(tile.texture_id, tile.bounds, bytes);
        lock.tiles_by_key.insert(key.clone(), tile.clone());

        Ok(Some(tile))
    }

    fn remove(&self, key: &AtlasKey) {
        let mut lock = self.0.lock();

        let Some(id) = lock.tiles_by_key.remove(key).map(|tile| tile.texture_id) else {
            return;
        };

        let Some(texture_slot) = lock.storage[id.kind].textures.get_mut(id.index as usize) else {
            return;
        };

        if let Some(mut texture) = texture_slot.take() {
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                if let Some(gl_texture) = texture.gl_texture {
                    lock.pending_deletions.push(PendingDeletion {
                        texture: gl_texture,
                    });
                }

                lock.storage[id.kind]
                    .free_list
                    .push(texture.id.index as usize);
            } else {
                *texture_slot = Some(texture);
            }
        }
    }
}

impl GlAtlasState {
    fn allocate(&mut self, size: Size<DevicePixels>, texture_kind: AtlasTextureKind) -> AtlasTile {
        {
            let textures = &mut self.storage[texture_kind];
            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return tile;
            }
        }

        let texture = self.push_texture(size, texture_kind);
        texture.allocate(size).unwrap()
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> &mut GlAtlasTexture {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(4096),
            height: DevicePixels(4096),
        };

        let size = min_size.max(&DEFAULT_ATLAS_SIZE);
        // OpenGL 2.1 uses ALPHA for single channel and RGBA for 4-channel.
        let format = match kind {
            AtlasTextureKind::Monochrome => GlTextureFormat::Alpha,
            AtlasTextureKind::Polychrome => GlTextureFormat::Rgba,
        };

        let texture_list = &mut self.storage[kind];
        let index = texture_list.free_list.pop();
        let new_index = index.unwrap_or(texture_list.textures.len()) as u32;

        let id = AtlasTextureId {
            index: new_index,
            kind,
        };

        self.pending_creations
            .push(PendingTextureCreation { id, size, kind });

        let atlas_texture = GlAtlasTexture {
            id,
            allocator: BucketedAtlasAllocator::new(size.into()),
            gl_texture: None,
            format,
            size,
            live_atlas_keys: 0,
        };

        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list.textures.get_mut(ix).unwrap().as_mut().unwrap()
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list.textures.last_mut().unwrap().as_mut().unwrap()
        }
    }

    fn upload_texture(
        &mut self,
        id: AtlasTextureId,
        bounds: Bounds<DevicePixels>,
        bytes: Cow<[u8]>,
    ) {
        self.pending_uploads.push(PendingUpload {
            id,
            bounds,
            data: bytes.into_owned(),
        });
    }

    fn flush(&mut self, gl: &glow::Context) {
        for deletion in self.pending_deletions.drain(..) {
            unsafe {
                gl.delete_texture(deletion.texture);
            }
        }

        for creation in self.pending_creations.drain(..) {
            let internal_format;
            let format;

            match creation.kind {
                AtlasTextureKind::Monochrome => {
                    // OpenGL 2.1: Use GL_ALPHA for single channel textures.
                    internal_format = glow::ALPHA;
                    format = glow::ALPHA;
                }
                AtlasTextureKind::Polychrome => {
                    internal_format = glow::RGBA;
                    format = glow::RGBA;
                }
            }

            unsafe {
                let texture = gl.create_texture().expect("Failed to create texture");
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));

                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::LINEAR as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_S,
                    glow::CLAMP_TO_EDGE as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_T,
                    glow::CLAMP_TO_EDGE as i32,
                );
                // FIX: Explicitly set MAX_LEVEL to 0 to signal no mipmaps.
                // This prevents "incomplete texture" state on some drivers.
                gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAX_LEVEL, 0);

                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    internal_format as i32,
                    creation.size.width.0,
                    creation.size.height.0,
                    0,
                    format,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );

                if let Some(tex_entry) = self.storage[creation.kind]
                    .textures
                    .get_mut(creation.id.index as usize)
                    .and_then(|t| t.as_mut())
                {
                    tex_entry.gl_texture = Some(texture);
                }
            }
        }

        for upload in self.pending_uploads.drain(..) {
            let texture_entry = &self.storage[upload.id];

            if let Some(gl_texture) = texture_entry.gl_texture {
                let format = match texture_entry.format {
                    GlTextureFormat::Alpha => glow::ALPHA,
                    GlTextureFormat::Rgba => glow::RGBA,
                };

                unsafe {
                    gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
                    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

                    gl.tex_sub_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        upload.bounds.origin.x.0,
                        upload.bounds.origin.y.0,
                        upload.bounds.size.width.0,
                        upload.bounds.size.height.0,
                        format,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(&upload.data)),
                    );
                }
            }
        }
    }
}

struct GlAtlasTexture {
    id: AtlasTextureId,
    allocator: BucketedAtlasAllocator,
    gl_texture: Option<glow::Texture>,
    format: GlTextureFormat,
    size: Size<DevicePixels>,
    live_atlas_keys: u32,
}

impl GlAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let padding = 1;
        let padded_size =
            etagere::euclid::Size2D::new(size.width.0 + padding * 2, size.height.0 + padding * 2);

        let allocation = self.allocator.allocate(padded_size)?;
        self.live_atlas_keys += 1;

        let origin = allocation.rectangle.min;

        Some(AtlasTile {
            texture_id: self.id,
            tile_id: allocation.id.into(),
            padding: 0,
            bounds: Bounds {
                origin: Point {
                    x: DevicePixels(origin.x + padding),
                    y: DevicePixels(origin.y + padding),
                },
                size,
            },
        })
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys -= 1;
    }

    fn is_unreferenced(&mut self) -> bool {
        self.live_atlas_keys == 0
    }
}

#[derive(Default)]
struct GlAtlasStorage {
    monochrome_textures: AtlasTextureList<GlAtlasTexture>,
    polychrome_textures: AtlasTextureList<GlAtlasTexture>,
}

impl ops::Index<AtlasTextureKind> for GlAtlasStorage {
    type Output = AtlasTextureList<GlAtlasTexture>;
    fn index(&self, kind: AtlasTextureKind) -> &Self::Output {
        match kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
        }
    }
}

impl ops::IndexMut<AtlasTextureKind> for GlAtlasStorage {
    fn index_mut(&mut self, kind: AtlasTextureKind) -> &mut Self::Output {
        match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        }
    }
}

impl ops::Index<AtlasTextureId> for GlAtlasStorage {
    type Output = GlAtlasTexture;
    fn index(&self, id: AtlasTextureId) -> &Self::Output {
        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
        };
        textures[id.index as usize].as_ref().unwrap()
    }
}
