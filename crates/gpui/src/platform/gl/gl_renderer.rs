use std::ffi::CString;
use std::mem;
use std::num::NonZeroU32;
use std::sync::Arc;

use glow::HasContext;
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version,
};
use glutin::display::{Display, DisplayApiPreference, GlDisplay};
use glutin::prelude::{GlSurface, PossiblyCurrentGlContext};
use glutin::surface::{Surface as GlutinSurface, SurfaceAttributesBuilder, WindowSurface};

use crate::{
    platform::gl::GlAtlas, DevicePixels, GpuSpecs, MonochromeSprite, PolychromeSprite,
    PrimitiveBatch, Quad, ScaledPixels, Scene, Shadow, Size,
};

pub struct GlSurfaceConfig {
    pub width: u32,
    pub height: u32,
    pub transparent: bool,
}

struct SpriteProgram {
    program: glow::Program,
    u_viewport_size: Option<glow::UniformLocation>,
    u_atlas_size: Option<glow::UniformLocation>,
    u_atlas: Option<glow::UniformLocation>,
}

struct QuadProgram {
    program: glow::Program,
    u_viewport_size: Option<glow::UniformLocation>,
}

struct ShadowProgram {
    program: glow::Program,
    u_viewport_size: Option<glow::UniformLocation>,
}

struct PolygonProgram {
    program: glow::Program,
    u_viewport_size: Option<glow::UniformLocation>,
}

pub struct GlRenderer {
    gl: Arc<glow::Context>,
    atlas: Arc<GlAtlas>,
    viewport_size: Size<DevicePixels>,
    _context: PossiblyCurrentContext,
    _surface: GlutinSurface<WindowSurface>,

    // VAO is required for Core Profile contexts (often default on Desktop)
    // even if we ask for GLES 2.0.
    vao: Option<glow::VertexArray>,

    mono_program: SpriteProgram,
    poly_program: SpriteProgram,
    quad_program: QuadProgram,
    shadow_program: ShadowProgram,
    polygon_program: PolygonProgram,

    index_buffer: glow::Buffer,
    dynamic_vertex_buffer: glow::Buffer,

    transparent: bool,
}

// Reduced batch size to 1024 sprites (4096 vertices) to be safe for embedded drivers
const MAX_SPRITES_PER_BATCH: usize = 1024;

impl GlRenderer {
    pub fn new<I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle>(
        config: GlSurfaceConfig,
        window: &I,
    ) -> anyhow::Result<Self> {
        let display_handle = window
            .display_handle()
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .as_raw();
        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .as_raw();

        let display = unsafe {
            Display::new(display_handle, DisplayApiPreference::Egl)
                .map_err(|e| anyhow::anyhow!("Failed to create EGL display: {}", e))?
        };

        let template = ConfigTemplateBuilder::new().with_alpha_size(8).build();

        let gl_config = unsafe { display.find_configs(template) }
            .map_err(|e| anyhow::anyhow!("Failed to discover OpenGL configs: {}", e))?
            .find(|c| c.depth_size() == 24)
            .ok_or_else(|| anyhow::anyhow!("Couldn't find suitable 24-bit depth config!"))?;

        log::info!(
            "Selected GL Config: api={:?}, depth={}, alpha={}",
            gl_config.api(),
            gl_config.depth_size(),
            gl_config.alpha_size()
        );

        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(Version::new(2, 0))))
            .build(Some(window_handle));

        let not_current_context = unsafe {
            display
                .create_context(&gl_config, &context_attributes)
                .map_err(|e| anyhow::anyhow!("Failed to create context: {}", e))?
        };

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window_handle,
            NonZeroU32::new(config.width.max(1)).unwrap(),
            NonZeroU32::new(config.height.max(1)).unwrap(),
        );
        let surface = unsafe { display.create_window_surface(&gl_config, &attrs)? };
        let context = not_current_context.make_current(&surface)?;

        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let s = CString::new(s).unwrap();
                display.get_proc_address(&s)
            })
        };
        let gl = Arc::new(gl);
        let atlas = Arc::new(GlAtlas::new());

        let version_str = unsafe { gl.get_parameter_string(glow::VERSION) };
        log::info!("GL Version String: {}", version_str);

        let (
            vao,
            mono_program,
            poly_program,
            quad_program,
            shadow_program,
            polygon_program,
            index_buffer,
            dynamic_vertex_buffer,
        ) = unsafe {
            // FIX: Create a Vertex Array Object.
            // On many Desktop GL drivers, even when requesting GLES, a VAO is required to draw.
            // We use .ok() to return None if the driver genuinely doesn't support/require them (pure GLES 2.0).
            let vao = gl.create_vertex_array().ok();

            // Bind it immediately so subsequent buffer bindings are associated with it (if stateful)
            // or just to ensure validity during setup.
            if let Some(vao) = vao {
                gl.bind_vertex_array(Some(vao));
            }

            let mono = create_sprite_program(&gl, false)?;
            let poly = create_sprite_program(&gl, true)?;
            let quad = create_quad_program(&gl)?;
            let shadow = create_shadow_program(&gl)?;
            let polygon = create_polygon_program(&gl)?;

            // 1. Create and populate a static Index Buffer for drawing quads
            let index_buf = gl.create_buffer().map_err(|e| anyhow::anyhow!(e))?;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buf));

            let mut indices: Vec<u16> = Vec::with_capacity(MAX_SPRITES_PER_BATCH * 6);
            for i in 0..MAX_SPRITES_PER_BATCH {
                let v = (i * 4) as u16;
                indices.extend_from_slice(&[v, v + 1, v + 2, v + 2, v + 1, v + 3]);
            }
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(&indices),
                glow::STATIC_DRAW,
            );

            // 2. Create the dynamic vertex buffer (will be streamed per frame)
            let dyn_buf = gl.create_buffer().map_err(|e| anyhow::anyhow!(e))?;

            // Setup clear color to transparent black
            gl.clear_color(0.0, 0.0, 0.0, 0.0);

            // Important: Force driver to complete initialization commands now.
            // This prevents "blank frame" issues where the first draw call happens
            // while shaders/buffers are still being allocated asynchronously.
            gl.finish();

            // Unbind VAO after setup to be clean
            if vao.is_some() {
                gl.bind_vertex_array(None);
            }

            (vao, mono, poly, quad, shadow, polygon, index_buf, dyn_buf)
        };

        Ok(Self {
            gl,
            atlas,
            viewport_size: Size {
                width: DevicePixels(config.width as i32),
                height: DevicePixels(config.height as i32),
            },
            _context: context,
            _surface: surface,
            vao,
            mono_program,
            poly_program,
            quad_program,
            shadow_program,
            polygon_program,
            index_buffer,
            dynamic_vertex_buffer,
            transparent: config.transparent,
        })
    }

    pub fn draw(&mut self, scene: &Scene) {
        // FIX: Prevent divide-by-zero in shader if viewport is invalid or minimized
        if self.viewport_size.width.0 <= 0 || self.viewport_size.height.0 <= 0 {
            return;
        }

        if let Err(e) = self._context.make_current(&self._surface) {
            log::error!("Make current failed: {}", e);
            return;
        }

        self.atlas.before_frame(&self.gl);

        unsafe {
            // FIX: Bind the VAO at the start of the frame.
            self.gl.bind_vertex_array(self.vao);

            self.gl.viewport(
                0,
                0,
                self.viewport_size.width.0,
                self.viewport_size.height.0,
            );
            self.gl.enable(glow::BLEND);

            // FIX: Use separate blend function for Alpha channel.
            self.gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );

            // Note: Depth testing is not enabled, so we only clear COLOR and Depth (just in case)
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            // Ensure buffers are bound to the current VAO (or context if no VAO)
            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.index_buffer));
            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.dynamic_vertex_buffer));
        }

        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites,
                } => self.draw_monochrome_sprites(texture_id, sprites),
                PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites,
                } => self.draw_polychrome_sprites(texture_id, sprites),
                PrimitiveBatch::Quads(quads) => self.draw_quads(quads),
                PrimitiveBatch::Polygons(polygons) => self.draw_polygons(polygons),
                PrimitiveBatch::Shadows(shadows) => self.draw_shadows(shadows),
                _ => {}
            }
        }

        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
            self.gl.bind_vertex_array(None);
        }

        if let Err(e) = self._surface.swap_buffers(&self._context) {
            log::error!("Swap buffers failed: {}", e);
        }
    }

    fn draw_batch<T>(
        &self,
        items: &[T],
        program: &glow::Program,
        attrib_setup_fn: impl Fn(&Self, i32),
        vertex_expansion_fn: impl Fn(&T, &mut Vec<f32>),
        floats_per_vertex: usize,
    ) {
        unsafe {
            self.gl.use_program(Some(*program));

            for chunk in items.chunks(MAX_SPRITES_PER_BATCH) {
                let mut vertices = Vec::with_capacity(chunk.len() * 4 * floats_per_vertex);

                for item in chunk {
                    vertex_expansion_fn(item, &mut vertices);
                }

                // Upload data to the currently bound ARRAY_BUFFER
                self.gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&vertices),
                    glow::DYNAMIC_DRAW,
                );

                let stride = (floats_per_vertex * mem::size_of::<f32>()) as i32;
                attrib_setup_fn(self, stride);

                let count = (chunk.len() * 6) as i32;
                self.gl
                    .draw_elements(glow::TRIANGLES, count, glow::UNSIGNED_SHORT, 0);
            }
        }
    }

    fn draw_monochrome_sprites(
        &mut self,
        texture_id: crate::AtlasTextureId,
        sprites: &[MonochromeSprite],
    ) {
        if sprites.is_empty() {
            return;
        }
        let Some(texture_info) = self.atlas.get_texture_info(texture_id) else {
            return;
        };

        unsafe {
            // FIX: Bind program before setting uniforms
            self.gl.use_program(Some(self.mono_program.program));

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(texture_info.texture));

            if let Some(loc) = &self.mono_program.u_atlas {
                self.gl.uniform_1_i32(Some(loc), 0);
            }
            if let Some(loc) = &self.mono_program.u_viewport_size {
                self.gl.uniform_2_f32(
                    Some(loc),
                    self.viewport_size.width.0 as f32,
                    self.viewport_size.height.0 as f32,
                );
            }
            if let Some(loc) = &self.mono_program.u_atlas_size {
                // FIX: Use actual texture size, not hardcoded 1024.0
                self.gl.uniform_2_f32(
                    Some(loc),
                    texture_info.size.width.0 as f32,
                    texture_info.size.height.0 as f32,
                );
            }

            let quad_coords = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

            self.draw_batch(
                sprites,
                &self.mono_program.program,
                |r, stride| r.bind_sprite_attribs(stride, false),
                |sprite, out| {
                    let c = sprite.color.to_rgb();
                    let rs = sprite.transformation.rotation_scale;
                    let t = sprite.transformation.translation;

                    for qc in quad_coords {
                        out.push(qc[0]);
                        out.push(qc[1]);
                        out.push(sprite.bounds.origin.x.0);
                        out.push(sprite.bounds.origin.y.0);
                        out.push(sprite.bounds.size.width.0);
                        out.push(sprite.bounds.size.height.0);
                        out.push(sprite.content_mask.bounds.origin.x.0);
                        out.push(sprite.content_mask.bounds.origin.y.0);
                        out.push(sprite.content_mask.bounds.size.width.0);
                        out.push(sprite.content_mask.bounds.size.height.0);
                        out.push(c.r);
                        out.push(c.g);
                        out.push(c.b);
                        out.push(c.a);
                        out.push(sprite.tile.bounds.origin.x.0 as f32);
                        out.push(sprite.tile.bounds.origin.y.0 as f32);
                        out.push(sprite.tile.bounds.size.width.0 as f32);
                        out.push(sprite.tile.bounds.size.height.0 as f32);
                        out.push(rs[0][0]);
                        out.push(rs[0][1]);
                        out.push(rs[1][0]);
                        out.push(rs[1][1]);
                        out.push(t[0]);
                        out.push(t[1]);
                    }
                },
                2 + 22,
            );
        }
    }

    fn draw_polychrome_sprites(
        &mut self,
        texture_id: crate::AtlasTextureId,
        sprites: &[PolychromeSprite],
    ) {
        if sprites.is_empty() {
            return;
        }
        let Some(texture_info) = self.atlas.get_texture_info(texture_id) else {
            return;
        };

        unsafe {
            // FIX: Bind program before setting uniforms
            self.gl.use_program(Some(self.poly_program.program));

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(texture_info.texture));

            if let Some(loc) = &self.poly_program.u_atlas {
                self.gl.uniform_1_i32(Some(loc), 0);
            }
            if let Some(loc) = &self.poly_program.u_viewport_size {
                self.gl.uniform_2_f32(
                    Some(loc),
                    self.viewport_size.width.0 as f32,
                    self.viewport_size.height.0 as f32,
                );
            }
            if let Some(loc) = &self.poly_program.u_atlas_size {
                // FIX: Use actual texture size
                self.gl.uniform_2_f32(
                    Some(loc),
                    texture_info.size.width.0 as f32,
                    texture_info.size.height.0 as f32,
                );
            }

            let quad_coords = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

            self.draw_batch(
                sprites,
                &self.poly_program.program,
                |r, stride| r.bind_sprite_attribs(stride, true),
                |sprite, out| {
                    for qc in quad_coords {
                        out.push(qc[0]);
                        out.push(qc[1]);
                        out.push(sprite.bounds.origin.x.0);
                        out.push(sprite.bounds.origin.y.0);
                        out.push(sprite.bounds.size.width.0);
                        out.push(sprite.bounds.size.height.0);
                        out.push(sprite.content_mask.bounds.origin.x.0);
                        out.push(sprite.content_mask.bounds.origin.y.0);
                        out.push(sprite.content_mask.bounds.size.width.0);
                        out.push(sprite.content_mask.bounds.size.height.0);
                        out.push(if sprite.grayscale { 1.0 } else { 0.0 });
                        out.push(sprite.opacity);
                        out.push(0.0);
                        out.push(0.0);
                        out.push(sprite.tile.bounds.origin.x.0 as f32);
                        out.push(sprite.tile.bounds.origin.y.0 as f32);
                        out.push(sprite.tile.bounds.size.width.0 as f32);
                        out.push(sprite.tile.bounds.size.height.0 as f32);
                    }
                },
                2 + 16,
            );
        }
    }

    fn draw_quads(&mut self, quads: &[Quad]) {
        if quads.is_empty() {
            return;
        }
        unsafe {
            // FIX: Bind program before setting uniforms
            self.gl.use_program(Some(self.quad_program.program));

            if let Some(loc) = &self.quad_program.u_viewport_size {
                self.gl.uniform_2_f32(
                    Some(loc),
                    self.viewport_size.width.0 as f32,
                    self.viewport_size.height.0 as f32,
                );
            }

            let quad_coords = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

            self.draw_batch(
                quads,
                &self.quad_program.program,
                |r, stride| r.bind_quad_attribs(stride),
                |quad, out| {
                    let b_rgba = quad.border_color.to_rgb();
                    let bg = &quad.background;
                    let (bg_tag, bg_space) = (bg.tag as u32 as f32, bg.color_space as u32 as f32);
                    let (c0r, c0g, c0b, c0a);
                    let (c1r, c1g, c1b, c1a);

                    if (bg.tag as u32) == 1 {
                        let c0 = bg.colors[0].color.to_rgb();
                        let c1 = bg.colors[1].color.to_rgb();
                        (c0r, c0g, c0b, c0a) = (c0.r, c0.g, c0.b, c0.a);
                        (c1r, c1g, c1b, c1a) = (c1.r, c1.g, c1.b, c1.a);
                    } else {
                        let solid = bg.solid.to_rgb();
                        (c0r, c0g, c0b, c0a) = (solid.r, solid.g, solid.b, solid.a);
                        (c1r, c1g, c1b, c1a) = (0.0, 0.0, 0.0, 0.0);
                    }

                    for qc in quad_coords {
                        out.push(qc[0]);
                        out.push(qc[1]);
                        out.push(quad.bounds.origin.x.0);
                        out.push(quad.bounds.origin.y.0);
                        out.push(quad.bounds.size.width.0);
                        out.push(quad.bounds.size.height.0);
                        out.push(quad.content_mask.bounds.origin.x.0);
                        out.push(quad.content_mask.bounds.origin.y.0);
                        out.push(quad.content_mask.bounds.size.width.0);
                        out.push(quad.content_mask.bounds.size.height.0);
                        out.push(quad.corner_radii.top_left.0);
                        out.push(quad.corner_radii.top_right.0);
                        out.push(quad.corner_radii.bottom_right.0);
                        out.push(quad.corner_radii.bottom_left.0);
                        out.push(quad.border_widths.top.0);
                        out.push(quad.border_widths.right.0);
                        out.push(quad.border_widths.bottom.0);
                        out.push(quad.border_widths.left.0);
                        out.push(b_rgba.r);
                        out.push(b_rgba.g);
                        out.push(b_rgba.b);
                        out.push(b_rgba.a);
                        out.push(bg_tag);
                        out.push(bg_space);
                        out.push(bg.gradient_angle_or_pattern_height);
                        out.push(bg.colors[0].percentage);
                        out.push(c0r);
                        out.push(c0g);
                        out.push(c0b);
                        out.push(c0a);
                        out.push(c1r);
                        out.push(c1g);
                        out.push(c1b);
                        out.push(c1a);
                        out.push(bg.colors[1].percentage);
                        out.push(quad.border_style as i32 as f32);
                        out.push(0.0);
                        out.push(0.0);
                    }
                },
                2 + 36,
            );
        }
    }

    fn draw_shadows(&mut self, shadows: &[Shadow]) {
        if shadows.is_empty() {
            return;
        }

        unsafe {
            // FIX: Bind program before setting uniforms
            self.gl.use_program(Some(self.shadow_program.program));

            if let Some(loc) = &self.shadow_program.u_viewport_size {
                self.gl.uniform_2_f32(
                    Some(loc),
                    self.viewport_size.width.0 as f32,
                    self.viewport_size.height.0 as f32,
                );
            }

            let quad_coords = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

            self.draw_batch(
                shadows,
                &self.shadow_program.program,
                |r, stride| r.bind_shadow_attribs(stride),
                |shadow, out| {
                    let c = shadow.color.to_rgb();
                    for qc in quad_coords {
                        out.push(qc[0]); // a_unit_quad.x
                        out.push(qc[1]); // a_unit_quad.y
                        out.push(shadow.bounds.origin.x.0); // a_bounds.x
                        out.push(shadow.bounds.origin.y.0); // a_bounds.y
                        out.push(shadow.bounds.size.width.0); // a_bounds.z
                        out.push(shadow.bounds.size.height.0); // a_bounds.w
                        out.push(shadow.content_mask.bounds.origin.x.0); // a_mask.x
                        out.push(shadow.content_mask.bounds.origin.y.0); // a_mask.y
                        out.push(shadow.content_mask.bounds.size.width.0); // a_mask.z
                        out.push(shadow.content_mask.bounds.size.height.0); // a_mask.w
                        out.push(shadow.corner_radii.top_left.0); // a_corner_radii.x
                        out.push(shadow.corner_radii.top_right.0); // a_corner_radii.y
                        out.push(shadow.corner_radii.bottom_right.0); // a_corner_radii.z
                        out.push(shadow.corner_radii.bottom_left.0); // a_corner_radii.w
                        out.push(c.r); // a_color.r
                        out.push(c.g); // a_color.g
                        out.push(c.b); // a_color.b
                        out.push(c.a); // a_color.a
                        out.push(shadow.blur_radius.0); // a_blur_radius
                    }
                },
                2 + 4 + 4 + 4 + 4 + 1, // Total floats per vertex: 19
            );
        }
    }

    fn draw_polygons(&mut self, polygons: &[crate::Polygon<ScaledPixels>]) {
        if polygons.is_empty() {
            return;
        }

        unsafe {
            self.gl.use_program(Some(self.polygon_program.program));

            if let Some(loc) = &self.polygon_program.u_viewport_size {
                self.gl.uniform_2_f32(
                    Some(loc),
                    self.viewport_size.width.0 as f32,
                    self.viewport_size.height.0 as f32,
                );
            }

            // Estimate capacity: Fill (approx 3N) + Border (approx 6N to 12N depending on joins)
            let estimated_capacity = polygons.iter().map(|p| p.points.len() * 15 * 20).sum();
            let mut vertices = Vec::with_capacity(estimated_capacity);

            for polygon in polygons {
                if polygon.points.len() < 3 {
                    continue;
                }

                let bg_color = polygon.background.solid.to_rgb();
                let border_color = polygon.border_color.to_rgb();
                let border_width = polygon.border_width.0;

                // --- 1. CLEAN INPUT ---
                // Remove consecutive duplicates and extremely close points to avoid NaN in normals
                // Fix: Collect to Vec first, then dedup in place
                let mut clean_points: Vec<(f32, f32)> = polygon
                    .points
                    .iter()
                    .map(|p| (p.x.0 as f32, p.y.0 as f32))
                    .collect();
                clean_points.dedup();

                if clean_points.len() < 3 {
                    continue;
                }

                let len = clean_points.len();

                // --- 2. FILL PASS (EARCUTR) ---
                let flat_coords: Vec<f64> = clean_points
                    .iter()
                    .flat_map(|(x, y)| [*x as f64, *y as f64])
                    .collect();

                // Compute Signed Area to determine winding
                // Sum (x2 - x1) * (y2 + y1). Result is 2*Area.
                // In Y-Down (Screen), Positive Area = Clockwise.
                // In Y-Down, "Standard" math (convex hull) usually results in CW.
                let mut signed_area = 0.0;
                for i in 0..len {
                    let (x1, y1) = clean_points[i];
                    let (x2, y2) = clean_points[(i + 1) % len];
                    signed_area += (x2 - x1) * (y2 + y1);
                }

                if let Ok(indices) = earcutr::earcut(&flat_coords, &[], 2) {
                    let mut push_vertex_fill = |x: f32, y: f32| {
                        vertices.push(x);
                        vertices.push(y);
                        vertices.push(polygon.bounds.origin.x.0);
                        vertices.push(polygon.bounds.origin.y.0);
                        vertices.push(polygon.bounds.size.width.0);
                        vertices.push(polygon.bounds.size.height.0);
                        vertices.push(polygon.content_mask.bounds.origin.x.0);
                        vertices.push(polygon.content_mask.bounds.origin.y.0);
                        vertices.push(polygon.content_mask.bounds.size.width.0);
                        vertices.push(polygon.content_mask.bounds.size.height.0);
                        vertices.push(border_color.r);
                        vertices.push(border_color.g);
                        vertices.push(border_color.b);
                        vertices.push(border_color.a);
                        vertices.push(0.0); // Border Width (0 = fill)
                        vertices.push(bg_color.r);
                        vertices.push(bg_color.g);
                        vertices.push(bg_color.b);
                        vertices.push(bg_color.a);
                        vertices.push(0.0); // Border Pos
                    };

                    for chunk in indices.chunks(3) {
                        if chunk.len() == 3 {
                            let (x0, y0) = clean_points[chunk[0]];
                            let (x1, y1) = clean_points[chunk[1]];
                            let (x2, y2) = clean_points[chunk[2]];
                            push_vertex_fill(x0, y0);
                            push_vertex_fill(x1, y1);
                            push_vertex_fill(x2, y2);
                        }
                    }
                }

                // --- 3. BORDER PASS (ROBUST OFFSET) ---
                if border_width > 0.0 {
                    // Determine Inset Direction:
                    // If Signed Area > 0 (CW in Y-down), the interior is "Right" of the edge.
                    // Standard Normal (-dy, dx) points "Left".
                    // So we need negative normal.
                    // If Signed Area < 0 (CCW in Y-down), interior is "Left" (Normal).
                    let inset_sign = if signed_area > 0.0 { -1.0 } else { 1.0 };

                    // Precompute normalized edge directions and normals
                    let mut edge_normals: Vec<(f32, f32)> = Vec::with_capacity(len);
                    for i in 0..len {
                        let (x1, y1) = clean_points[i];
                        let (x2, y2) = clean_points[(i + 1) % len];
                        let dx = x2 - x1;
                        let dy = y2 - y1;
                        // Fix: Explicitly infer length type
                        let length = (dx * dx + dy * dy).sqrt();

                        if length < 1e-4 {
                            // Degenerate edge (should be rare due to dedup)
                            edge_normals.push((0.0, 0.0));
                        } else {
                            // Normal (-dy, dx) points Left
                            edge_normals.push((-dy / length, dx / length));
                        }
                    }

                    // Calculate Inner Corner Points
                    // For each corner i, we compute the "End of Inner Edge i-1" and "Start of Inner Edge i"
                    // If Miter is valid, these are the same point.
                    // If Bevel, they are different.
                    let mut inner_points = Vec::with_capacity(len);

                    for i in 0..len {
                        let curr = clean_points[i];
                        let prev_idx = (i + len - 1) % len;
                        let curr_idx = i;

                        let n_prev = edge_normals[prev_idx];
                        let n_curr = edge_normals[curr_idx];

                        // Robust Miter / Bevel Logic
                        // We are offsetting lines by (Normal * sign * width).
                        // Miter point is intersection of two offset lines.
                        
                        // Check angle via dot product
                        let dot = n_prev.0 * n_curr.0 + n_prev.1 * n_curr.1;
                        // Miter limit check: 1.0 / sin(theta/2).
                        // Conservative limit to switch to bevel
                        let miter_limit = 3.0; 
                        
                        // Check if effectively parallel (0 or 180)
                        let is_parallel = (dot - 1.0).abs() < 1e-4 || (dot + 1.0).abs() < 1e-4;
                        
                        // Calculate standard offset vectors
                        let off_x_prev = n_prev.0 * border_width * inset_sign;
                        let off_y_prev = n_prev.1 * border_width * inset_sign;
                        let off_x_curr = n_curr.0 * border_width * inset_sign;
                        let off_y_curr = n_curr.1 * border_width * inset_sign;

                        if is_parallel {
                             // Just use the normals directly (Bevel/Flat join logic)
                             // End of Prev Inner
                             let p1 = (curr.0 + off_x_prev, curr.1 + off_y_prev);
                             // Start of Curr Inner
                             let p2 = (curr.0 + off_x_curr, curr.1 + off_y_curr);
                             inner_points.push((p1, p2));
                        } else {
                            // Miter vector calculation
                            // u = (n1 + n2).normalized()
                            // miter_len = width / dot(u, n1)
                            
                            let sum_x = n_prev.0 + n_curr.0;
                            let sum_y = n_prev.1 + n_curr.1;
                            let sum_len = (sum_x*sum_x + sum_y*sum_y).sqrt();
                            
                            let k = if sum_len > 1e-4 {
                                let u_x = sum_x / sum_len;
                                let u_y = sum_y / sum_len;
                                let dot_u_n = u_x * n_prev.0 + u_y * n_prev.1;
                                if dot_u_n.abs() > 1e-4 {
                                    1.0 / dot_u_n
                                } else {
                                    miter_limit + 1.0 // Force bevel
                                }
                            } else {
                                miter_limit + 1.0 // Force bevel
                            };

                            if k.abs() <= miter_limit {
                                // Valid Miter
                                let miter_x = (sum_x / sum_len) * k * border_width * inset_sign;
                                let miter_y = (sum_y / sum_len) * k * border_width * inset_sign;
                                let p = (curr.0 + miter_x, curr.1 + miter_y);
                                inner_points.push((p, p));
                            } else {
                                // Bevel: Use separate offset points
                                let p1 = (curr.0 + off_x_prev, curr.1 + off_y_prev);
                                let p2 = (curr.0 + off_x_curr, curr.1 + off_y_curr);
                                inner_points.push((p1, p2));
                            }
                        }
                    }

                    // Generate Geometry
                    let mut push_vertex_border = |x: f32, y: f32, border_pos: f32| {
                         vertices.push(x);
                         vertices.push(y);
                         vertices.push(polygon.bounds.origin.x.0);
                         vertices.push(polygon.bounds.origin.y.0);
                         vertices.push(polygon.bounds.size.width.0);
                         vertices.push(polygon.bounds.size.height.0);
                         vertices.push(polygon.content_mask.bounds.origin.x.0);
                         vertices.push(polygon.content_mask.bounds.origin.y.0);
                         vertices.push(polygon.content_mask.bounds.size.width.0);
                         vertices.push(polygon.content_mask.bounds.size.height.0);
                         vertices.push(border_color.r);
                         vertices.push(border_color.g);
                         vertices.push(border_color.b);
                         vertices.push(border_color.a);
                         vertices.push(border_width);
                         vertices.push(0.0); // BG unused
                         vertices.push(0.0);
                         vertices.push(0.0);
                         vertices.push(0.0);
                         vertices.push(border_pos); // 0.0=Outer, 1.0=Inner
                    };

                    for i in 0..len {
                        let curr_v = clean_points[i];
                        let next_v = clean_points[(i + 1) % len];
                        
                        // 1. Join Geometry (At Vertex i)
                        // Connects Inner_End of Prev Edge to Inner_Start of Curr Edge
                        let (inner_prev_end, inner_curr_start) = inner_points[i];
                        
                        // If points are different, we have a Bevel triangle
                        if (inner_prev_end.0 - inner_curr_start.0).abs() > 1e-4 || 
                           (inner_prev_end.1 - inner_curr_start.1).abs() > 1e-4 {
                            // Triangle: Outer(i) -> Inner_Prev_End -> Inner_Curr_Start
                            push_vertex_border(curr_v.0, curr_v.1, 0.0);
                            push_vertex_border(inner_prev_end.0, inner_prev_end.1, 1.0);
                            push_vertex_border(inner_curr_start.0, inner_curr_start.1, 1.0);
                        }

                        // 2. Segment Geometry (Edge i)
                        // Connects i to i+1
                        // Outer: curr_v -> next_v
                        // Inner: inner_curr_start -> inner_next_end
                        let (inner_next_end, _) = inner_points[(i + 1) % len];
                        
                        // Quad as 2 triangles
                        // T1: OuterCurr, OuterNext, InnerNextEnd
                        push_vertex_border(curr_v.0, curr_v.1, 0.0);
                        push_vertex_border(next_v.0, next_v.1, 0.0);
                        push_vertex_border(inner_next_end.0, inner_next_end.1, 1.0);
                        
                        // T2: OuterCurr, InnerNextEnd, InnerCurrStart
                        push_vertex_border(curr_v.0, curr_v.1, 0.0);
                        push_vertex_border(inner_next_end.0, inner_next_end.1, 1.0);
                        push_vertex_border(inner_curr_start.0, inner_curr_start.1, 1.0);
                    }
                }
            }

            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::DYNAMIC_DRAW,
            );

            // 20 floats * 4 bytes
            let stride = 20 * mem::size_of::<f32>() as i32;
            self.bind_polygon_attribs(stride);

            // Calculate total vertices
            let total_vertices = vertices.len() / 20;
            self.gl
                .draw_arrays(glow::TRIANGLES, 0, total_vertices as i32);
        }
    }

    unsafe fn bind_sprite_attribs(&self, stride: i32, is_polychrome: bool) {
        unsafe {
            let f32_size = mem::size_of::<f32>() as i32;
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            let base_offset = 2 * f32_size;
            for (idx, size, offset) in [
                (1, 4, 0),
                (2, 4, 4 * f32_size),
                (3, 4, 8 * f32_size),
                (4, 4, 12 * f32_size),
            ] {
                self.gl.enable_vertex_attrib_array(idx);
                self.gl.vertex_attrib_pointer_f32(
                    idx,
                    size,
                    glow::FLOAT,
                    false,
                    stride,
                    base_offset + offset,
                );
            }

            if !is_polychrome {
                self.gl.enable_vertex_attrib_array(5);
                self.gl.vertex_attrib_pointer_f32(
                    5,
                    4,
                    glow::FLOAT,
                    false,
                    stride,
                    base_offset + 16 * f32_size,
                );
                self.gl.enable_vertex_attrib_array(6);
                self.gl.vertex_attrib_pointer_f32(
                    6,
                    2,
                    glow::FLOAT,
                    false,
                    stride,
                    base_offset + 20 * f32_size,
                );
            } else {
                self.gl.disable_vertex_attrib_array(5);
                self.gl.disable_vertex_attrib_array(6);
            }
            self.gl.disable_vertex_attrib_array(7);
            self.gl.disable_vertex_attrib_array(8);
            self.gl.disable_vertex_attrib_array(9);
        }
    }

    fn bind_quad_attribs(&self, stride: i32) {
        unsafe {
            let f32_size = mem::size_of::<f32>() as i32;
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            let base_offset = 2 * f32_size;
            for i in 1..=9 {
                self.gl.enable_vertex_attrib_array(i);
                self.gl.vertex_attrib_pointer_f32(
                    i,
                    4,
                    glow::FLOAT,
                    false,
                    stride,
                    base_offset + (i as i32 - 1) * 4 * f32_size,
                );
            }
        }
    }

    fn bind_shadow_attribs(&self, stride: i32) {
        unsafe {
            let f32_size = mem::size_of::<f32>() as i32;
            // a_unit_quad (2)
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            let base_offset = 2 * f32_size;

            // a_bounds (4)
            self.gl.enable_vertex_attrib_array(1);
            self.gl
                .vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, stride, base_offset);

            // a_mask (4)
            self.gl.enable_vertex_attrib_array(2);
            self.gl.vertex_attrib_pointer_f32(
                2,
                4,
                glow::FLOAT,
                false,
                stride,
                base_offset + 4 * f32_size,
            );

            // a_corner_radii (4)
            self.gl.enable_vertex_attrib_array(3);
            self.gl.vertex_attrib_pointer_f32(
                3,
                4,
                glow::FLOAT,
                false,
                stride,
                base_offset + 8 * f32_size,
            );

            // a_color (4)
            self.gl.enable_vertex_attrib_array(4);
            self.gl.vertex_attrib_pointer_f32(
                4,
                4,
                glow::FLOAT,
                false,
                stride,
                base_offset + 12 * f32_size,
            );

            // a_blur_radius (1)
            self.gl.enable_vertex_attrib_array(5);
            self.gl.vertex_attrib_pointer_f32(
                5,
                1,
                glow::FLOAT,
                false,
                stride,
                base_offset + 16 * f32_size,
            );

            // Disable unused attributes from other programs to be safe
            for i in 6..=9 {
                self.gl.disable_vertex_attrib_array(i);
            }
        }
    }

    fn bind_polygon_attribs(&self, stride: i32) {
        unsafe {
            let f32_size = mem::size_of::<f32>() as i32;

            // a_position (2)
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            let base_offset = 2 * f32_size;

            // a_bounds (4)
            self.gl.enable_vertex_attrib_array(1);
            self.gl
                .vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, stride, base_offset);

            // a_mask (4)
            self.gl.enable_vertex_attrib_array(2);
            self.gl.vertex_attrib_pointer_f32(
                2,
                4,
                glow::FLOAT,
                false,
                stride,
                base_offset + 4 * f32_size,
            );

            // a_border_color (4)
            self.gl.enable_vertex_attrib_array(3);
            self.gl.vertex_attrib_pointer_f32(
                3,
                4,
                glow::FLOAT,
                false,
                stride,
                base_offset + 8 * f32_size,
            );

            // a_border_width (1)
            self.gl.enable_vertex_attrib_array(4);
            self.gl.vertex_attrib_pointer_f32(
                4,
                1,
                glow::FLOAT,
                false,
                stride,
                base_offset + 12 * f32_size,
            );

            // a_bg_color (4)
            self.gl.enable_vertex_attrib_array(5);
            self.gl.vertex_attrib_pointer_f32(
                5,
                4,
                glow::FLOAT,
                false,
                stride,
                base_offset + 13 * f32_size,
            );

            // a_border_pos (1)
            self.gl.enable_vertex_attrib_array(6);
            self.gl.vertex_attrib_pointer_f32(
                6,
                1,
                glow::FLOAT,
                false,
                stride,
                base_offset + 17 * f32_size,
            );

            // Disable unused attributes
            for i in 7..=9 {
                self.gl.disable_vertex_attrib_array(i);
            }
        }
    }

    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        self.viewport_size = size;
        let width = NonZeroU32::new(size.width.0 as u32).unwrap_or(NonZeroU32::MIN);
        let height = NonZeroU32::new(size.height.0 as u32).unwrap_or(NonZeroU32::MIN);
        self._surface.resize(&self._context, width, height);
    }

    pub fn update_transparency(&mut self, transparency: bool) {
        self.transparent = transparency;
    }

    pub fn sprite_atlas(&self) -> &std::sync::Arc<GlAtlas> {
        &self.atlas
    }

    pub fn gpu_specs(&self) -> GpuSpecs {
        let renderer = unsafe { self.gl.get_parameter_string(glow::RENDERER) };
        let version = unsafe { self.gl.get_parameter_string(glow::VERSION) };
        GpuSpecs {
            is_software_emulated: false, // Hard to detect reliably without specific vendor checks
            device_name: renderer,
            driver_name: "OpenGL".to_string(),
            driver_info: version,
        }
    }

    pub fn destroy(&mut self) {
        let _ = self._context.make_current(&self._surface);
        unsafe {
            if let Some(vao) = self.vao {
                self.gl.delete_vertex_array(vao);
            }
            self.gl.delete_program(self.mono_program.program);
            self.gl.delete_program(self.poly_program.program);
            self.gl.delete_program(self.quad_program.program);
            self.gl.delete_program(self.shadow_program.program);
            self.gl.delete_program(self.polygon_program.program);
            self.gl.delete_buffer(self.index_buffer);
            self.gl.delete_buffer(self.dynamic_vertex_buffer);
        }
    }
}

fn create_sprite_program(gl: &glow::Context, is_polychrome: bool) -> anyhow::Result<SpriteProgram> {
    unsafe {
        let vs_source = format!(
            r#"#version 100
            precision highp float;
            attribute vec2 a_unit_quad;
            attribute vec4 a_bounds;
            attribute vec4 a_mask;
            attribute vec4 a_color_or_params;
            attribute vec4 a_tile_uv;
            {extra_layout}
            uniform vec2 u_viewport_size;
            uniform vec2 u_atlas_size;
            
            varying vec2 v_uv;
            varying vec4 v_color_params;
            varying vec2 v_screen_pos;
            varying vec4 v_mask;

            void main() {{
                vec2 initial_pos = a_bounds.xy + a_unit_quad * a_bounds.zw;
                vec2 pos = initial_pos;
                {transform}
                v_screen_pos = pos;
                // FIX: Guard against div-by-zero handled in CPU, but careful here
                vec2 ndc = (pos / u_viewport_size) * 2.0 - 1.0;
                ndc.y = -ndc.y;
                gl_Position = vec4(ndc, 0.0, 1.0);
                vec2 uv_px = a_tile_uv.xy + a_unit_quad * a_tile_uv.zw;
                v_uv = uv_px / u_atlas_size;
                v_color_params = a_color_or_params;
                v_mask = a_mask;
            }}
        "#,
            extra_layout = if is_polychrome {
                String::new()
            } else {
                "attribute vec4 a_transform_rs;\nattribute vec2 a_transform_t;".to_string()
            },
            transform = if is_polychrome {
                ""
            } else {
                "pos = vec2(dot(a_transform_rs.xy, initial_pos), dot(a_transform_rs.zw, initial_pos)) + a_transform_t;"
            }
        );

        let fs_source = format!(
            r#"#version 100
            precision mediump float;
            varying vec2 v_uv;
            varying vec4 v_color_params;
            varying vec2 v_screen_pos;
            varying vec4 v_mask;
            
            uniform sampler2D u_atlas;

            void main() {{
                if (v_screen_pos.x < v_mask.x || v_screen_pos.y < v_mask.y || 
                    v_screen_pos.x > v_mask.x + v_mask.z || v_screen_pos.y > v_mask.y + v_mask.w) {{
                    discard;
                }}
                vec4 tex = texture2D(u_atlas, v_uv);
                {color_logic}
                gl_FragColor = result;
            }}
        "#,
            color_logic = if is_polychrome {
                r#"
                float grayscale = v_color_params.x;
                float opacity = v_color_params.y;
                if (grayscale > 0.5) {
                    float gray = dot(tex.rgb, vec3(0.299, 0.587, 0.114));
                    tex = vec4(vec3(gray), tex.a);
                }
                vec4 result = tex * opacity;
                "#
            } else {
                r#"
                float alpha = tex.a;
                vec4 result = vec4(v_color_params.rgb, v_color_params.a * alpha);
                "#
            }
        );

        let mut attribs = vec![
            (0, "a_unit_quad"),
            (1, "a_bounds"),
            (2, "a_mask"),
            (3, "a_color_or_params"),
            (4, "a_tile_uv"),
        ];
        if !is_polychrome {
            attribs.push((5, "a_transform_rs"));
            attribs.push((6, "a_transform_t"));
        }

        let program = compile_program(gl, &vs_source, &fs_source, &attribs)?;
        Ok(SpriteProgram {
            program,
            u_viewport_size: gl.get_uniform_location(program, "u_viewport_size"),
            u_atlas_size: gl.get_uniform_location(program, "u_atlas_size"),
            u_atlas: gl.get_uniform_location(program, "u_atlas"),
        })
    }
}

fn create_quad_program(gl: &glow::Context) -> anyhow::Result<QuadProgram> {
    let vs_source = r#"#version 100
        precision highp float;
        attribute vec2 a_unit_quad;
        attribute vec4 a_bounds; 
        attribute vec4 a_mask;
        attribute vec4 a_corner_radii; 
        attribute vec4 a_border_widths; 
        attribute vec4 a_border_color;
        attribute vec4 a_bg_info; 
        attribute vec4 a_bg_color0; 
        attribute vec4 a_bg_color1; 
        attribute vec4 a_params;

        uniform vec2 u_viewport_size;

        varying vec4 v_geom; 
        varying vec4 v_mask_dist;
        varying vec4 v_corner_radii; 
        varying vec4 v_border_widths; 
        varying vec4 v_border_color;
        varying vec4 v_style_info; 
        varying vec4 v_bg_color0; 
        varying vec4 v_bg_color1;

        void main() {
            vec2 size = a_bounds.zw;
            vec2 half_size = size * 0.5;
            vec2 center = a_bounds.xy + half_size;
            vec2 pos = a_bounds.xy + a_unit_quad * size;
            v_geom = vec4(pos - center, half_size);
            v_mask_dist = vec4(pos.x - a_mask.x, pos.y - a_mask.y, (a_mask.x + a_mask.z) - pos.x, (a_mask.y + a_mask.w) - pos.y);
            v_corner_radii = a_corner_radii; v_border_widths = a_border_widths; v_border_color = a_border_color;
            v_bg_color0 = a_bg_color0; v_bg_color1 = a_bg_color1;
            v_style_info = vec4(a_bg_info.x + a_bg_info.y * 10.0, a_bg_info.z, a_bg_info.w, a_params.x);
            vec2 ndc = (pos / u_viewport_size) * 2.0 - 1.0;
            ndc.y = -ndc.y;
            gl_Position = vec4(ndc, 0.0, 1.0);
        }
    "#;

    let fs_source = r#"#version 100
        precision mediump float;
        
        float trunc(float x) { return sign(x)*floor(abs(x)); }
        float round(float x) { return floor(x+0.5); }

        varying vec4 v_geom; 
        varying vec4 v_mask_dist;
        varying vec4 v_corner_radii; 
        varying vec4 v_border_widths; 
        varying vec4 v_border_color;
        varying vec4 v_style_info; 
        varying vec4 v_bg_color0; 
        varying vec4 v_bg_color1;

        float fmod(float a, float b) { return a - b * trunc(a / b); }
        
        float pick_corner_radius(vec2 center_to_point, vec4 radii) {
            if (center_to_point.x < 0.0) { return (center_to_point.y < 0.0) ? radii.x : radii.w; }
            else { return (center_to_point.y < 0.0) ? radii.y : radii.z; }
        }

        float quad_sdf_impl(vec2 corner_center_to_point, float corner_radius) {
            if (corner_radius == 0.0) { return max(corner_center_to_point.x, corner_center_to_point.y); }
            return length(max(vec2(0.0), corner_center_to_point)) + min(0.0, max(corner_center_to_point.x, corner_center_to_point.y)) - corner_radius;
        }

        float quarter_ellipse_sdf(vec2 point, vec2 radii) {
             vec2 circle_vec = point / radii;
             return (length(circle_vec) - 1.0) * (radii.x + radii.y) * -0.5;
        }

        void main() {
            if (v_mask_dist.x < 0.0 || v_mask_dist.y < 0.0 || v_mask_dist.z < 0.0 || v_mask_dist.w < 0.0) { discard; }
            vec2 center_to_point = v_geom.xy;
            vec2 half_size = v_geom.zw;
            float corner_radius = pick_corner_radius(center_to_point, v_corner_radii);
            
            float packed_flags = floor(v_style_info.x + 0.5);
            int tag = int(mod(packed_flags, 10.0) + 0.1);
            int space = int(mod(floor(packed_flags / 10.0), 10.0) + 0.1);

            vec4 bg_color = vec4(0.0);
            if (tag == 0 || tag == 2) { bg_color = v_bg_color0; } 
            else if (tag == 1) {
                float angle = v_style_info.y;
                float stop0 = v_style_info.z;
                float stop1 = v_style_info.w;
                float rad = (fmod(angle, 360.0) - 90.0) * 3.14159 / 180.0;
                vec2 dir = vec2(cos(rad), sin(rad));
                if (half_size.x > half_size.y) { dir.y *= half_size.y / half_size.x; } else { dir.x *= half_size.x / half_size.y; }
                float t = dot(center_to_point, dir) / length(dir);
                if (abs(dir.x) > abs(dir.y)) { t = (t + half_size.x) / (half_size.x * 2.0); } else { t = (t + half_size.y) / (half_size.y * 2.0); }
                t = clamp((t - stop0) / (stop1 - stop0), 0.0, 1.0);
                
                bg_color = mix(v_bg_color0, v_bg_color1, t);
            }

            bool unrounded = (v_corner_radii == vec4(0.0));
            bool no_border = (v_border_widths == vec4(0.0));
            if (unrounded && no_border) { gl_FragColor = bg_color; return; }

            float antialias = 0.5;
            vec2 border = vec2((center_to_point.x < 0.0) ? v_border_widths.w : v_border_widths.y, (center_to_point.y < 0.0) ? v_border_widths.x : v_border_widths.z);
            vec2 reduced_border = vec2((border.x == 0.0) ? -antialias : border.x, (border.y == 0.0) ? -antialias : border.y);
            vec2 corner_to_point = abs(center_to_point) - half_size;
            vec2 corner_center_to_point = corner_to_point + corner_radius;
            float outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);
            
            float inner_sdf = 0.0;
            vec2 straight_inner = corner_to_point + reduced_border;
            if (corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0) { inner_sdf = -max(straight_inner.x, straight_inner.y); }
            else if (straight_inner.x > 0.0 || straight_inner.y > 0.0) { inner_sdf = -1.0; }
            else if (reduced_border.x == reduced_border.y) { inner_sdf = -(outer_sdf + reduced_border.x); }
            else { inner_sdf = quarter_ellipse_sdf(corner_center_to_point, max(vec2(0.0), corner_radius - reduced_border)); }

            float border_sdf = max(inner_sdf, outer_sdf);
            vec4 color = bg_color;
            if (border_sdf < antialias) {
                vec4 border_col = v_border_color;
                color = mix(color, border_col, clamp(antialias - inner_sdf, 0.0, 1.0));
            }
            float alpha_factor = clamp(antialias - outer_sdf, 0.0, 1.0);
            gl_FragColor = vec4(color.rgb, color.a * alpha_factor);
        }
    "#;

    let attribs = vec![
        (0, "a_unit_quad"),
        (1, "a_bounds"),
        (2, "a_mask"),
        (3, "a_corner_radii"),
        (4, "a_border_widths"),
        (5, "a_border_color"),
        (6, "a_bg_info"),
        (7, "a_bg_color0"),
        (8, "a_bg_color1"),
        (9, "a_params"),
    ];
    let program = compile_program(gl, vs_source, fs_source, &attribs)?;
    Ok(QuadProgram {
        program,
        u_viewport_size: unsafe { gl.get_uniform_location(program, "u_viewport_size") },
    })
}

fn create_shadow_program(gl: &glow::Context) -> anyhow::Result<ShadowProgram> {
    let vs_source = r#"#version 100
        precision highp float;
        attribute vec2 a_unit_quad;
        attribute vec4 a_bounds;
        attribute vec4 a_mask;
        attribute vec4 a_corner_radii;
        attribute vec4 a_color;
        attribute float a_blur_radius;

        uniform vec2 u_viewport_size;

        varying vec4 v_color;
        varying vec2 v_pos;      // Screen position (pixels)
        varying vec4 v_bounds;   // Original bounds (for corner calc)
        varying vec4 v_mask;
        varying vec4 v_corner_radii;
        varying float v_blur_radius;

        void main() {
            float margin = 3.0 * a_blur_radius;
            // Expand the bounds to account for the blur spread
            vec2 origin = a_bounds.xy - vec2(margin);
            vec2 size = a_bounds.zw + vec2(2.0 * margin);
            vec2 pos_px = origin + a_unit_quad * size;

            v_bounds = a_bounds;
            v_color = a_color;
            v_mask = a_mask;
            v_corner_radii = a_corner_radii;
            v_blur_radius = a_blur_radius;
            v_pos = pos_px;

            vec2 ndc = (pos_px / u_viewport_size) * 2.0 - 1.0;
            ndc.y = -ndc.y;
            gl_Position = vec4(ndc, 0.0, 1.0);
        }
    "#;

    let fs_source = r#"#version 100
        precision highp float;

        varying vec4 v_color;
        varying vec2 v_pos;
        varying vec4 v_bounds;
        varying vec4 v_mask;
        varying vec4 v_corner_radii;
        varying float v_blur_radius;

        // Approximation of the error function
        vec2 erf(vec2 x) {
            vec2 s = sign(x);
            vec2 a = abs(x);
            x = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
            x = x * x;
            return s - s / (x * x);
        }

        float gaussian(float x, float sigma) {
            return exp(-(x * x) / (2.0 * sigma * sigma)) / (2.506628 * sigma); // 2.506... is sqrt(2*PI)
        }

        float blur_along_x(float x, float y, float sigma, float corner, vec2 half_size) {
            float delta = min(half_size.y - corner - abs(y), 0.0);
            float curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
            vec2 integral = 0.5 + 0.5 * erf((vec2(x) + vec2(-curved, curved)) * (0.707106 / sigma)); // 0.707... is sqrt(0.5)
            return integral.y - integral.x;
        }

        float pick_corner_radius(vec2 center_to_point, vec4 radii) {
            if (center_to_point.x < 0.0) { return (center_to_point.y < 0.0) ? radii.x : radii.w; }
            else { return (center_to_point.y < 0.0) ? radii.y : radii.z; }
        }

        void main() {
            // Clip logic
            if (v_pos.x < v_mask.x || v_pos.y < v_mask.y || 
                v_pos.x > v_mask.x + v_mask.z || v_pos.y > v_mask.y + v_mask.w) {
                discard;
            }

            vec2 half_size = v_bounds.zw / 2.0;
            vec2 center = v_bounds.xy + half_size;
            vec2 center_to_point = v_pos - center;

            float corner_radius = pick_corner_radius(center_to_point, v_corner_radii);

            // Limit range of integration to where the signal is non-zero
            float low = center_to_point.y - half_size.y;
            float high = center_to_point.y + half_size.y;
            float start = clamp(-3.0 * v_blur_radius, low, high);
            float end = clamp(3.0 * v_blur_radius, low, high);

            // Integrate
            float step_size = (end - start) / 4.0;
            float y = start + step_size * 0.5;
            float alpha = 0.0;
            
            // Loop unrolled for GLES 2.0 safety/performance
            for (int i = 0; i < 4; i++) {
                float blur = blur_along_x(center_to_point.x, center_to_point.y - y, v_blur_radius, corner_radius, half_size);
                alpha += blur * gaussian(y, v_blur_radius) * step_size;
                y += step_size;
            }

            gl_FragColor = vec4(v_color.rgb, v_color.a * alpha);
        }
    "#;

    let attribs = vec![
        (0, "a_unit_quad"),
        (1, "a_bounds"),
        (2, "a_mask"),
        (3, "a_corner_radii"),
        (4, "a_color"),
        (5, "a_blur_radius"),
    ];
    let program = compile_program(gl, vs_source, fs_source, &attribs)?;
    Ok(ShadowProgram {
        program,
        u_viewport_size: unsafe { gl.get_uniform_location(program, "u_viewport_size") },
    })
}

fn create_polygon_program(gl: &glow::Context) -> anyhow::Result<PolygonProgram> {
    let vs_source = r#"#version 100
        precision highp float;
        attribute vec2 a_position;
        attribute vec4 a_bounds;
        attribute vec4 a_mask;
        attribute vec4 a_border_color;
        attribute float a_border_width;
        attribute vec4 a_bg_color;
        attribute float a_border_pos;

        uniform vec2 u_viewport_size;

        varying vec2 v_pos;
        varying vec4 v_bounds;
        varying vec4 v_mask;
        varying vec4 v_border_color;
        varying float v_border_width;
        varying vec4 v_bg_color;
        varying float v_border_pos;

        void main() {
            v_pos = a_position;
            v_bounds = a_bounds;
            v_mask = a_mask;
            v_border_color = a_border_color;
            v_border_width = a_border_width;
            v_bg_color = a_bg_color;
            v_border_pos = a_border_pos;

            vec2 ndc = (a_position / u_viewport_size) * 2.0 - 1.0;
            ndc.y = -ndc.y;
            gl_Position = vec4(ndc, 0.0, 1.0);
        }
    "#;

    let fs_source = r#"#version 100
        precision mediump float;

        varying vec2 v_pos;
        varying vec4 v_bounds;
        varying vec4 v_mask;
        varying vec4 v_border_color;
        varying float v_border_width;
        varying vec4 v_bg_color;
        varying float v_border_pos;

        void main() {
            if (v_pos.x < v_mask.x || v_pos.y < v_mask.y || 
                v_pos.x > v_mask.x + v_mask.z || v_pos.y > v_mask.y + v_mask.w) {
                discard;
            }

            vec4 color = v_bg_color;
            
            // If v_border_width > 0, we are rendering the border strip.
            // In the border pass, we send v_bg_color as transparent (0,0,0,0) and v_border_color as the actual color.
            // v_border_pos is 0.0 at the outer edge, 1.0 at the inner edge.
            
            if (v_border_width > 0.0) {
                float dist_px = v_border_pos * v_border_width;
                float dist_from_inner = v_border_width - dist_px;
                
                // Simple Anti-Aliasing at the inner edge
                // We assume the outer edge is hard (handled by polygon fill geometry) or we don't AA it to avoid gaps.
                // Smooth the alpha over the last 1.0 pixel of the width
                float alpha_factor = smoothstep(0.0, 1.0, dist_from_inner);
                
                color = vec4(v_border_color.rgb, v_border_color.a * alpha_factor);
            }

            gl_FragColor = color;
        }
    "#;

    let attribs = vec![
        (0, "a_position"),
        (1, "a_bounds"),
        (2, "a_mask"),
        (3, "a_border_color"),
        (4, "a_border_width"),
        (5, "a_bg_color"),
        (6, "a_border_pos"),
    ];

    let program = compile_program(gl, vs_source, fs_source, &attribs)?;
    Ok(PolygonProgram {
        program,
        u_viewport_size: unsafe { gl.get_uniform_location(program, "u_viewport_size") },
    })
}

fn compile_program(
    gl: &glow::Context,
    vs: &str,
    fs: &str,
    attribs: &[(u32, &str)],
) -> anyhow::Result<glow::Program> {
    unsafe {
        let p = gl.create_program().map_err(|e| anyhow::anyhow!(e))?;
        let v = gl
            .create_shader(glow::VERTEX_SHADER)
            .map_err(|e| anyhow::anyhow!(e))?;
        gl.shader_source(v, vs);
        gl.compile_shader(v);
        if !gl.get_shader_compile_status(v) {
            return Err(anyhow::anyhow!("VS: {}", gl.get_shader_info_log(v)));
        }
        gl.attach_shader(p, v);
        let f = gl
            .create_shader(glow::FRAGMENT_SHADER)
            .map_err(|e| anyhow::anyhow!(e))?;
        gl.shader_source(f, fs);
        gl.compile_shader(f);
        if !gl.get_shader_compile_status(f) {
            return Err(anyhow::anyhow!("FS: {}", gl.get_shader_info_log(f)));
        }
        gl.attach_shader(p, f);
        for (i, name) in attribs {
            gl.bind_attrib_location(p, *i, name);
        }
        gl.link_program(p);
        if !gl.get_program_link_status(p) {
            return Err(anyhow::anyhow!("Link: {}", gl.get_program_info_log(p)));
        }
        gl.delete_shader(v);
        gl.delete_shader(f);
        Ok(p)
    }
}
