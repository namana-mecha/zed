use std::{ffi::CString, num::NonZeroU32};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentGlContext as _, Version},
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext as _},
    surface::{GlSurface, SurfaceAttributesBuilder, WindowSurface},
};
// Explicitly import Impeller types to avoid ambiguity
use impellers::{
    BlendMode, ClipOperation, Color, ColorFilter, ColorMatrix, ColorSource, DisplayListBuilder,
    DrawStyle, FillType, ISize, ImageFilter, Matrix, Paint, PathBuilder,
    Point, Rect as ImpellerRect, RoundingRadii,
    Size as ImpellerSize, TextureSampling, TileMode,
};

// ImpellerPoint is a C struct with x, y fields but not publicly exposed
// We define our own compatible struct to match the C ABI
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
struct ImpellerPoint {
    x: f32,
    y: f32,
}

// Helper function to safely convert our ImpellerPoint to impellers::sys::ImpellerPoint
// This is safe because both structs have identical memory layout (#[repr(C)] with same fields)
fn to_sys_rounding_radii(radii: RoundingRadiiCompat) -> RoundingRadii {
    unsafe { std::mem::transmute(radii) }
}

// Compatible struct for building RoundingRadii
#[repr(C)]
struct RoundingRadiiCompat {
    top_left: ImpellerPoint,
    bottom_left: ImpellerPoint,
    top_right: ImpellerPoint,
    bottom_right: ImpellerPoint,
}

use crate::{
    color::BackgroundTag,
    platform::impeller::{ImpellerAtlas, ImpellerContext},
    GpuSpecs, PlatformRenderer, PrimitiveBatch,
};

// --- Helper Traits for Clean Conversions ---

// Fix: Add Generic Argument <crate::DevicePixels> to GPUI types
impl From<crate::Point<crate::DevicePixels>> for ImpellerPoint {
    #[inline(always)]
    fn from(p: crate::Point<crate::DevicePixels>) -> Self {
        ImpellerPoint {
            x: p.x.0 as f32,
            y: p.y.0 as f32,
        }
    }
}

impl From<crate::Size<crate::DevicePixels>> for ImpellerSize {
    #[inline(always)]
    fn from(s: crate::Size<crate::DevicePixels>) -> Self {
        Self::new(s.width.0 as f32, s.height.0 as f32)
    }
}

impl From<crate::Bounds<crate::DevicePixels>> for ImpellerRect {
    #[inline(always)]
    fn from(r: crate::Bounds<crate::DevicePixels>) -> Self {
        ImpellerRect::new(
            Point::new(r.origin.x.0 as f32, r.origin.y.0 as f32),
            ImpellerSize::new(r.size.width.0 as f32, r.size.height.0 as f32),
        )
    }
}

impl From<crate::Point<crate::ScaledPixels>> for ImpellerPoint {
    #[inline(always)]
    fn from(p: crate::Point<crate::ScaledPixels>) -> Self {
        ImpellerPoint {
            x: p.x.0,
            y: p.y.0,
        }
    }
}

impl From<crate::Size<crate::ScaledPixels>> for ImpellerSize {
    #[inline(always)]
    fn from(s: crate::Size<crate::ScaledPixels>) -> Self {
        Self::new(s.width.0, s.height.0)
    }
}

impl From<crate::Bounds<crate::ScaledPixels>> for ImpellerRect {
    #[inline(always)]
    fn from(r: crate::Bounds<crate::ScaledPixels>) -> Self {
        ImpellerRect::new(
            Point::new(r.origin.x.0, r.origin.y.0),
            ImpellerSize::new(r.size.width.0, r.size.height.0),
        )
    }
}

impl From<crate::Hsla> for Color {
    #[inline(always)]
    fn from(c: crate::Hsla) -> Self {
        let rgba = c.to_rgb();
        Self::new_srgba(rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

// Fix: Construct ImpellerPoints explicitly for Radii instead of relying on Into<(f32, f32)>
impl From<crate::Corners<crate::ScaledPixels>> for RoundingRadii {
    #[inline(always)]
    fn from(cr: crate::Corners<crate::ScaledPixels>) -> Self {
        to_sys_rounding_radii(RoundingRadiiCompat {
            top_left: ImpellerPoint { x: cr.top_left.0, y: cr.top_left.0 },
            bottom_left: ImpellerPoint { x: cr.bottom_left.0, y: cr.bottom_left.0 },
            top_right: ImpellerPoint { x: cr.top_right.0, y: cr.top_right.0 },
            bottom_right: ImpellerPoint { x: cr.bottom_right.0, y: cr.bottom_right.0 },
        })
    }
}

pub struct ImpellerRenderer {
    sprite_atlas: std::sync::Arc<ImpellerAtlas>,
    framebuffer: Option<impellers::Surface>,
    gl_surface: glutin::surface::Surface<WindowSurface>,
    gl_context: glutin::context::PossiblyCurrentContext,
    impeller_context: impellers::Context,
    transparent: bool,
    is_context_active: bool,
    viewport_size: (u32, u32),
}

impl ImpellerRenderer {
    pub fn new<I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle>(
        _context: &ImpellerContext,
        window: &I,
        config: (u32, u32),
    ) -> anyhow::Result<Self> {
        let gl_display = unsafe {
            glutin::display::Display::new(
                window
                    .display_handle()
                    .expect("Unable to get display handle from window")
                    .as_raw(),
                glutin::display::DisplayApiPreference::Egl,
            )
        }
        .expect("Unable to create glutin display");

        let template = ConfigTemplateBuilder::new().with_alpha_size(8).build();

        let gl_config = unsafe { gl_display.find_configs(template) }
            .unwrap()
            .reduce(|config, acc| {
                if config.num_samples() > acc.num_samples() {
                    config
                } else {
                    acc
                }
            })
            .expect("No available configs");

        let raw_window_handle = window.window_handle().unwrap().as_raw();
        let context_attributes = ContextAttributesBuilder::new().build(Some(raw_window_handle));

        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let legacy_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(2, 1))))
            .build(Some(raw_window_handle));

        let gl_display = gl_config.display();

        let not_current_gl_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_display
                        .create_context(&gl_config, &fallback_context_attributes)
                        .unwrap_or_else(|_| {
                            gl_display
                                .create_context(&gl_config, &legacy_context_attributes)
                                .expect("Unable to create GL context")
                        })
                })
        };

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            NonZeroU32::new(config.0.max(1)).unwrap(),
            NonZeroU32::new(config.1.max(1)).unwrap(),
        );

        let gl_surface = unsafe { gl_display.create_window_surface(&gl_config, &attrs)? };

        let gl_context = not_current_gl_context.make_current(&gl_surface)?;
        
        // Fix: Make context mutable for wrapping FBO
        let mut impeller_context: impellers::Context = unsafe {
            impellers::Context::new_opengl_es(|s| {
                gl_context
                    .display()
                    .get_proc_address(CString::new(s).unwrap().as_c_str()) as _
            })
        }
        .unwrap();

        let sprite_atlas = std::sync::Arc::new(ImpellerAtlas::new());
        sprite_atlas.set_context(impeller_context.clone());

        // Fix: .get() and cast to i64 for ISize
        let width_i64 = config.0.max(1) as i64;
        let height_i64 = config.1.max(1) as i64;

        let framebuffer = unsafe {
            impeller_context
                .wrap_fbo(
                    0,
                    impellers::PixelFormat::RGBA8888,
                    ISize::new(width_i64, height_i64),
                )
                .unwrap()
        };

        Ok(Self {
            sprite_atlas,
            gl_context,
            impeller_context,
            gl_surface,
            framebuffer: Some(framebuffer),
            transparent: false,
            is_context_active: true,
            viewport_size: config,
        })
    }

    fn ensure_context_current(&mut self) {
        if !self.is_context_active || !self.gl_context.is_current() {
            self.gl_context
                .make_current(&self.gl_surface)
                .expect("Failed to make GL context current");
            self.is_context_active = true;
        }
    }
}

impl PlatformRenderer for ImpellerRenderer {
    type RenderParams = (u32, u32);

    fn draw(&mut self, scene: &crate::Scene) {
        self.ensure_context_current();
        self.sprite_atlas.before_frame(&self.impeller_context);

        let mut builder = DisplayListBuilder::new(None);
        let mut paint = Paint::default();
        let mut aux_paint = Paint::default(); 

        if self.transparent {
            paint.set_color(Color::new_srgba(0.0, 0.0, 0.0, 0.0));
        } else {
            paint.set_color(Color::BLACKBERRY);
        }
        builder.draw_paint(&paint);

        let viewport_rect = ImpellerRect::new(
            Point::new(0.0, 0.0), 
            ImpellerSize::new(self.viewport_size.0 as f32, self.viewport_size.1 as f32)
        );

        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Quads(quads) => {
                    for q in quads.iter() {
                        let rect: ImpellerRect = q.bounds.into();

                        // Fix: Borrows &rect for intersection check
                        if !viewport_rect.intersects(&rect) {
                            continue;
                        }

                        // Apply content mask clipping
                        builder.save();
                        let content_mask_rect: ImpellerRect = q.content_mask.bounds.into();
                        builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                        let is_sharp_rect = q.corner_radii.top_left.0 == 0.0
                            && q.corner_radii.top_right.0 == 0.0
                            && q.corner_radii.bottom_left.0 == 0.0
                            && q.corner_radii.bottom_right.0 == 0.0;

                        paint.set_draw_style(DrawStyle::Fill);

                        match q.background.tag {
                            BackgroundTag::Solid => {
                                let hsl = q.background.solid;
                                // Fix: Access alpha field directly
                                if hsl.a > 0.001 {
                                    let color: Color = q.background.solid.into();
                                    paint.set_color(color);
                                    
                                    if is_sharp_rect {
                                        builder.draw_rect(&rect, &paint);
                                    } else {
                                        let radii: RoundingRadii = q.corner_radii.into();
                                        builder.draw_rounded_rect(&rect, &radii, &paint);
                                    }
                                }
                            }
                            BackgroundTag::LinearGradient => {
                                let angle = q.background.gradient_angle_or_pattern_height;
                                let origin = rect.origin;
                                let size = rect.size;

                                let (start, end) = if (angle - 180.0).abs() < 0.1 {
                                    (
                                        Point::new(origin.x + size.width / 2.0, origin.y),
                                        Point::new(origin.x + size.width / 2.0, origin.y + size.height),
                                    )
                                } else if (angle - 90.0).abs() < 0.1 {
                                    (
                                        Point::new(origin.x, origin.y + size.height / 2.0),
                                        Point::new(origin.x + size.width, origin.y + size.height / 2.0),
                                    )
                                } else {
                                    let angle_rad = angle.to_radians();
                                    let center_x = origin.x + size.width / 2.0;
                                    let center_y = origin.y + size.height / 2.0;
                                    let diagonal = (size.width.powi(2) + size.height.powi(2)).sqrt() / 2.0;
                                    (
                                        Point::new(
                                            center_x - angle_rad.sin() * diagonal,
                                            center_y - angle_rad.cos() * diagonal,
                                        ),
                                        Point::new(
                                            center_x + angle_rad.sin() * diagonal,
                                            center_y + angle_rad.cos() * diagonal,
                                        ),
                                    )
                                };

                                // Fix: Compare source gpui::Color because ImpellerColor !impl PartialEq
                                let gpui_c0 = q.background.colors[0].color;
                                let gpui_c1 = q.background.colors[1].color;
                                
                                let c0: Color = gpui_c0.into();
                                let c1: Color = gpui_c1.into();
                                
                                if gpui_c0 == gpui_c1 {
                                    paint.set_color(c0);
                                    if is_sharp_rect {
                                        builder.draw_rect(&rect, &paint);
                                    } else {
                                        let radii: RoundingRadii = q.corner_radii.into();
                                        builder.draw_rounded_rect(&rect, &radii, &paint);
                                    }
                                } else {
                                    let colors = [c0, c1];
                                    let stops = [
                                        q.background.colors[0].percentage,
                                        q.background.colors[1].percentage,
                                    ];
                                    let gradient = ColorSource::new_linear_gradient(
                                        start, end, &colors, &stops, TileMode::Clamp, None,
                                    );
                                    paint.set_color_source(&gradient);
                                    
                                    if is_sharp_rect {
                                        builder.draw_rect(&rect, &paint);
                                    } else {
                                        let radii: RoundingRadii = q.corner_radii.into();
                                        builder.draw_rounded_rect(&rect, &radii, &paint);
                                    }
                                    
                                    // Fix: Replace non-existent .invalid() with a fresh paint reset
                                    // or just set a transparent color to clear the source state 
                                    // (assuming Impeller behavior matches Skia where set_color overrides source)
                                    paint = Paint::default();
                                }
                            }
                            BackgroundTag::PatternSlash => {
                                let color: Color = q.background.solid.into();
                                paint.set_color(color);
                                let radii: RoundingRadii = q.corner_radii.into();
                                builder.draw_rounded_rect(&rect, &radii, &paint);
                            }
                        }

                        let has_border = q.border_widths.top.0 > 0.0
                            || q.border_widths.right.0 > 0.0
                            || q.border_widths.bottom.0 > 0.0
                            || q.border_widths.left.0 > 0.0;

                        if has_border {
                            let border_color: Color = q.border_color.into();
                            
                            // Fix: Use assignment instead of .reset()
                            aux_paint = Paint::default();
                            aux_paint.set_color(border_color);

                            let w = &q.border_widths;
                            let is_uniform = w.top == w.right && w.right == w.bottom && w.bottom == w.left;

                            if is_uniform {
                                aux_paint.set_draw_style(DrawStyle::Stroke);
                                aux_paint.set_stroke_width(w.top.0);
                                
                                if is_sharp_rect {
                                    builder.draw_rect(&rect, &aux_paint);
                                } else {
                                    let radii: RoundingRadii = q.corner_radii.into();
                                    builder.draw_rounded_rect(&rect, &radii, &aux_paint);
                                }
                            } else {
                                let inner_rect = ImpellerRect::new(
                                    Point::new(rect.origin.x + w.left.0, rect.origin.y + w.top.0),
                                    ImpellerSize::new(
                                        rect.size.width - w.left.0 - w.right.0,
                                        rect.size.height - w.top.0 - w.bottom.0,
                                    ),
                                );
                                
                                // Fix: Construct Points explicitly
                                let tl = (q.corner_radii.top_left.0 - w.left.0.max(w.top.0)).max(0.0);
                                let bl = (q.corner_radii.bottom_left.0 - w.left.0.max(w.bottom.0)).max(0.0);
                                let tr = (q.corner_radii.top_right.0 - w.right.0.max(w.top.0)).max(0.0);
                                let br = (q.corner_radii.bottom_right.0 - w.right.0.max(w.bottom.0)).max(0.0);
                                let inner_radii = to_sys_rounding_radii(RoundingRadiiCompat {
                                    top_left: ImpellerPoint { x: tl, y: tl },
                                    bottom_left: ImpellerPoint { x: bl, y: bl },
                                    top_right: ImpellerPoint { x: tr, y: tr },
                                    bottom_right: ImpellerPoint { x: br, y: br },
                                });
                                let outer_radii: RoundingRadii = q.corner_radii.into();

                                builder.draw_rounded_rect_difference(
                                    &rect, &outer_radii, &inner_rect, &inner_radii, &aux_paint,
                                );
                            }
                        }

                        builder.restore();
                    }
                }
                PrimitiveBatch::Polygons(polygons) => {
                    for polygon in polygons.iter() {
                        if polygon.points.is_empty() { continue; }

                        builder.save();
                        let content_mask_rect: ImpellerRect = polygon.content_mask.bounds.into();
                        builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                        let mut path_builder = PathBuilder::default();
                        let first = &polygon.points[0];
                        path_builder.move_to(Point::new(first.x.0, first.y.0));
                        for p in polygon.points.iter().skip(1) {
                            path_builder.line_to(Point::new(p.x.0, p.y.0));
                        }
                        path_builder.close();
                        let impeller_path = path_builder.take_path_new(FillType::NonZero);

                        paint.set_draw_style(DrawStyle::Fill);
                        match polygon.background.tag {
                            BackgroundTag::Solid => {
                                paint.set_color(polygon.background.solid.into());
                                builder.draw_path(&impeller_path, &paint);
                            }
                            _ => { }
                        }

                        if polygon.border_width.0 > 0.0 {
                            aux_paint = Paint::default();
                            aux_paint.set_color(polygon.border_color.into());
                            aux_paint.set_stroke_width(polygon.border_width.0);
                            aux_paint.set_draw_style(DrawStyle::Stroke);
                            builder.draw_path(&impeller_path, &aux_paint);
                        }

                        builder.restore();
                    }
                }
                PrimitiveBatch::Paths(paths) => {
                    for path in paths.iter() {
                        if path.vertices.is_empty() { continue; }

                        builder.save();
                        let content_mask_rect: ImpellerRect = path.content_mask.bounds.into();
                        builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                        let mut path_builder = PathBuilder::default();
                        let origin = path.bounds.origin;
                        let size = path.bounds.size;
                        path_builder.add_rect(&ImpellerRect::new(
                            Point::new(origin.x.0, origin.y.0),
                            ImpellerSize::new(size.width.0, size.height.0),
                        ));
                        let impeller_path = path_builder.take_path_new(FillType::NonZero);

                        paint.set_color(path.color.solid.into());
                        builder.draw_path(&impeller_path, &paint);

                        builder.restore();
                    }
                }
                PrimitiveBatch::Shadows(shadows) => {
                    for shadow in shadows.iter() {
                        let rect: ImpellerRect = shadow.bounds.into();
                        // Fix: Borrow check for intersection
                        if !viewport_rect.intersects(&rect) { continue; }

                        builder.save();
                        let content_mask_rect: ImpellerRect = shadow.content_mask.bounds.into();
                        builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                        let radii: RoundingRadii = shadow.corner_radii.into();
                        let blur_sigma = shadow.blur_radius.0 / 2.0;
                        let spread = shadow.blur_radius.0 * 3.0;

                        let shadow_rect = ImpellerRect::new(
                            Point::new(rect.origin.x - spread, rect.origin.y - spread),
                            ImpellerSize::new(rect.size.width + 2.0 * spread, rect.size.height + 2.0 * spread),
                        );

                        aux_paint = Paint::default();
                        aux_paint.set_color(shadow.color.into());
                        
                        if blur_sigma > 0.0 {
                            let blur_filter = ImageFilter::new_blur(blur_sigma, blur_sigma, TileMode::Clamp);
                            aux_paint.set_image_filter(&blur_filter);
                        }
                        builder.draw_rounded_rect(&shadow_rect, &radii, &aux_paint);

                        builder.restore();
                    }
                }
                PrimitiveBatch::Underlines(underlines) => {
                    for underline in underlines.iter() {
                        let rect: ImpellerRect = underline.bounds.into();

                        builder.save();
                        let content_mask_rect: ImpellerRect = underline.content_mask.bounds.into();
                        builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                        paint.set_color(underline.color.into());

                        if underline.wavy != 0 {
                            let y = rect.origin.y + rect.size.height / 2.0;
                            let wave_length = underline.thickness.0 * 4.0;
                            builder.draw_dashed_line(
                                Point::new(rect.origin.x, y),
                                Point::new(rect.origin.x + rect.size.width, y),
                                wave_length, wave_length / 2.0, &paint
                            );
                        } else {
                            builder.draw_rect(&ImpellerRect::new(
                                rect.origin,
                                ImpellerSize::new(rect.size.width, underline.thickness.0)
                            ), &paint);
                        }

                        builder.restore();
                    }
                }
                PrimitiveBatch::MonochromeSprites { texture_id, sprites } => {
                     if let Some(texture) = self.sprite_atlas.get_texture(texture_id) {
                        for sprite in sprites.iter() {
                            let rect: ImpellerRect = sprite.bounds.into();
                            if !viewport_rect.intersects(&rect) { continue; }

                            builder.save();  // For clipping
                            let content_mask_rect: ImpellerRect = sprite.content_mask.bounds.into();
                            builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                            let color: Color = sprite.color.into();
                            
                            aux_paint = Paint::default();
                            let color_filter = ColorFilter::new_blend(color, BlendMode::SourceIn);
                            aux_paint.set_color_filter(&color_filter);

                            let tile_bounds = sprite.tile.bounds;
                            let src_rect = ImpellerRect::new(
                                Point::new(tile_bounds.origin.x.0 as f32, tile_bounds.origin.y.0 as f32),
                                ImpellerSize::new(tile_bounds.size.width.0 as f32, tile_bounds.size.height.0 as f32),
                            );

                            let transform = sprite.transformation;
                            let is_identity = transform.rotation_scale == [[1.0, 0.0], [0.0, 1.0]];
                            
                            if is_identity {
                                let tx = transform.translation[0];
                                let ty = transform.translation[1];
                                let dst_rect = ImpellerRect::new(
                                    Point::new(rect.origin.x + tx, rect.origin.y + ty),
                                    rect.size
                                );
                                builder.draw_texture_rect(
                                    &texture, &src_rect, &dst_rect, TextureSampling::Linear, Some(&aux_paint)
                                );
                            } else {
                                builder.save();
                                let matrix = Matrix::new(
                                    transform.rotation_scale[0][0], transform.rotation_scale[1][0], 0.0, 0.0,
                                    transform.rotation_scale[0][1], transform.rotation_scale[1][1], 0.0, 0.0,
                                    0.0, 0.0, 1.0, 0.0,
                                    transform.translation[0], transform.translation[1], 0.0, 1.0,
                                );
                                builder.transform(&matrix);
                                builder.draw_texture_rect(
                                    &texture, &src_rect, &rect, TextureSampling::Linear, Some(&aux_paint)
                                );
                                builder.restore();
                            }

                            builder.restore();  // Restore clipping
                        }
                     }
                }
                PrimitiveBatch::PolychromeSprites { texture_id, sprites } => {
                    if let Some(texture) = self.sprite_atlas.get_texture(texture_id) {
                         for sprite in sprites.iter() {
                            let rect: ImpellerRect = sprite.bounds.into();
                            if !viewport_rect.intersects(&rect) { continue; }

                            builder.save();
                            let content_mask_rect: ImpellerRect = sprite.content_mask.bounds.into();
                            builder.clip_rect(&content_mask_rect, ClipOperation::Intersect);

                            aux_paint = Paint::default();
                            aux_paint.set_blend_mode(BlendMode::SourceOver);
                            if sprite.grayscale {
                                let grayscale_matrix = ColorMatrix {
                                    m: [
                                        0.2126, 0.7152, 0.0722, 0.0, 0.0, 
                                        0.2126, 0.7152, 0.0722, 0.0, 0.0, 
                                        0.2126, 0.7152, 0.0722, 0.0, 0.0, 
                                        0.0, 0.0, 0.0, 1.0, 0.0,
                                    ],
                                };
                                let filter = ColorFilter::new_matrix(grayscale_matrix);
                                aux_paint.set_color_filter(&filter);
                            }

                            let tile_bounds = sprite.tile.bounds;
                             let src_rect = ImpellerRect::new(
                                Point::new(tile_bounds.origin.x.0 as f32, tile_bounds.origin.y.0 as f32),
                                ImpellerSize::new(tile_bounds.size.width.0 as f32, tile_bounds.size.height.0 as f32),
                            );

                            builder.draw_texture_rect(
                                &texture, &src_rect, &rect, TextureSampling::Linear, Some(&aux_paint)
                            );

                            builder.restore();
                         }
                    }
                }
                _ => {}
            }
        }

        if self.framebuffer.is_none() { return; }
        
        self.framebuffer.as_mut().unwrap()
            .draw_display_list(&builder.build().unwrap())
            .unwrap();
            
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("Failed to swap buffers");
    }

    fn sprite_atlas(&self) -> std::sync::Arc<dyn crate::PlatformAtlas> {
        self.sprite_atlas.clone()
    }

    fn gpu_specs(&self) -> crate::GpuSpecs {
        GpuSpecs {
            is_software_emulated: true,
            device_name: Default::default(),
            driver_name: Default::default(),
            driver_info: Default::default(),
        }
    }

    fn update_drawable_size(&mut self, size: crate::Size<crate::DevicePixels>) {
        self.ensure_context_current();

        let width = NonZeroU32::new((size.width.0 as u32).max(1)).unwrap();
        let height = NonZeroU32::new((size.height.0 as u32).max(1)).unwrap();

        self.gl_surface.resize(&self.gl_context, width, height);
        
        self.viewport_size = (width.into(), height.into());

        self.framebuffer = unsafe {
            self.impeller_context.wrap_fbo(
                0,
                impellers::PixelFormat::RGBA8888,
                // Fix: explicit i64 cast
                ISize::new(width.get() as i64, height.get() as i64),
            )
        };
    }

    fn update_transparency(&mut self, transparent: bool) {
        self.transparent = transparent;
    }

    fn destroy(&mut self) {
        self.framebuffer = None;
    }
}
