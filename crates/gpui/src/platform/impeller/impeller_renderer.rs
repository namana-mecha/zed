use std::{ffi::CString, num::NonZeroU32};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentGlContext as _, Version},
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext as _},
    surface::{GlSurface, SurfaceAttributesBuilder, WindowSurface},
};
use impellers::{
    BlendMode, ClipOperation, Color, ColorFilter, ColorMatrix, ColorSource, DisplayListBuilder,
    DrawStyle, FillType, ISize, ImageFilter, Matrix, Paint, PathBuilder, Point, Rect, Size,
    TextureSampling, TileMode,
};

use crate::{
    color::BackgroundTag, platform::impeller::{ImpellerAtlas, ImpellerContext}, GpuSpecs,
    PlatformRenderer, PrimitiveBatch,
};

pub struct ImpellerRenderer {
    sprite_atlas: std::sync::Arc<ImpellerAtlas>,
    framebuffer: Option<impellers::Surface>,
    gl_surface: glutin::surface::Surface<WindowSurface>,
    // Each window has its own GL context for multi-window support
    gl_context: glutin::context::PossiblyCurrentContext,
    impeller_context: impellers::Context,
    #[allow(dead_code)]
    glow_context: glow::Context,
    transparent: bool,
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

        let context_attributes =
            ContextAttributesBuilder::new().build(Some(window.window_handle().unwrap().as_raw()));

        // Since glutin by default tries to create OpenGL core context, which may not be
        // present we should try gles.
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(window.window_handle().unwrap().as_raw()));

        // There are also some old devices that support neither modern OpenGL nor GLES.
        // To support these we can try and create a 2.1 context.
        let legacy_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(2, 1))))
            .build(Some(window.window_handle().unwrap().as_raw()));

        // Reuse the uncurrented context from a suspended() call if it exists, otherwise
        // this is the first time resumed() is called, where the context still
        // has to be created.
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
            window.window_handle().unwrap().as_raw(),
            NonZeroU32::new(config.0.max(1)).unwrap(),
            NonZeroU32::new(config.1.max(1)).unwrap(),
        );

        let gl_surface = unsafe { gl_display.create_window_surface(&gl_config, &attrs)? };

        let gl_context = not_current_gl_context.make_current(&gl_surface)?;
        let mut impeller_context: impellers::Context = unsafe {
            impellers::Context::new_opengl_es(|s| {
                gl_context
                    .display()
                    .get_proc_address(CString::new(s).unwrap().as_c_str()) as _
            })
        }
        .unwrap();
        let glow_context: glow::Context = unsafe {
            glow::Context::from_loader_function(|s| {
                gl_context
                    .display()
                    .get_proc_address(CString::new(s).unwrap().as_c_str()) as _
            }) as _
        };

        let sprite_atlas = std::sync::Arc::new(ImpellerAtlas::new());
        sprite_atlas.set_context(impeller_context.clone());

        let framebuffer = unsafe {
            impeller_context
                .wrap_fbo(
                    0,
                    impellers::PixelFormat::RGBA8888,
                    ISize::new(config.0.max(1).into(), config.1.max(1).into()),
                )
                .unwrap()
        };

        Ok(Self {
            sprite_atlas,
            glow_context,
            gl_context,
            impeller_context,
            gl_surface,
            framebuffer: Some(framebuffer),
            transparent: false,
        })
    }
}
impl PlatformRenderer for ImpellerRenderer {
    type RenderParams = (u32, u32);

    fn draw(&mut self, scene: &crate::Scene) {
        // Make this context current before rendering
        // This is critical for multi-window support - each window has its own GL context
        // and we need to ensure the correct context is active before rendering
        if !self.gl_context.is_current() {
            self.gl_context
                .make_current(&self.gl_surface)
                .expect("Failed to make GL context current");
        }

        let mut builder = DisplayListBuilder::new(None);
        let mut paint = Paint::default();

        // Emulate transparency by drawing either a transparent or opaque background
        if self.transparent {
            paint.set_color(Color::new_srgba(0.0, 0.0, 0.0, 0.0));
        } else {
            paint.set_color(Color::BLACKBERRY);
        }
        builder.draw_paint(&paint);
        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Quads(quads) => {
                    for q in quads.iter() {
                        let origin = q.bounds.origin;
                        let size = q.bounds.size;

                        let radii: impellers::RoundingRadii = unsafe {
                            std::mem::transmute([
                                q.corner_radii.top_left.0,
                                q.corner_radii.top_left.0,
                                q.corner_radii.bottom_left.0,
                                q.corner_radii.bottom_left.0,
                                q.corner_radii.top_right.0,
                                q.corner_radii.top_right.0,
                                q.corner_radii.bottom_right.0,
                                q.corner_radii.bottom_right.0,
                            ])
                        };

                        let rect = Rect::new(
                            Point::new(origin.x.0, origin.y.0),
                            Size::new(size.width.0, size.height.0),
                        );

                        match q.background.tag {
                            BackgroundTag::Solid => {
                                let hsl_color = q.background.solid;
                                let rgba_color = hsl_color.to_rgb();
                                let color = Color::new_srgba(
                                    rgba_color.r,
                                    rgba_color.g,
                                    rgba_color.b,
                                    rgba_color.a,
                                );

                                paint.set_color(color);
                                builder.draw_rounded_rect(&rect, &radii, &paint);
                            }
                            BackgroundTag::LinearGradient => {
                                let angle = q.background.gradient_angle_or_pattern_height;
                                let angle_rad = angle.to_radians();

                                let center_x = origin.x.0 + size.width.0 / 2.0;
                                let center_y = origin.y.0 + size.height.0 / 2.0;

                                let diagonal =
                                    ((size.width.0 * size.width.0 + size.height.0 * size.height.0)
                                        .sqrt())
                                        / 2.0;

                                let start = Point::new(
                                    center_x - angle_rad.sin() * diagonal,
                                    center_y - angle_rad.cos() * diagonal,
                                );
                                let end = Point::new(
                                    center_x + angle_rad.sin() * diagonal,
                                    center_y + angle_rad.cos() * diagonal,
                                );

                                let color0 = q.background.colors[0].color.to_rgb();
                                let color1 = q.background.colors[1].color.to_rgb();

                                let colors = [
                                    Color::new_srgba(color0.r, color0.g, color0.b, color0.a),
                                    Color::new_srgba(color1.r, color1.g, color1.b, color1.a),
                                ];

                                let stops = [
                                    q.background.colors[0].percentage,
                                    q.background.colors[1].percentage,
                                ];

                                let gradient = ColorSource::new_linear_gradient(
                                    start,
                                    end,
                                    &colors,
                                    &stops,
                                    TileMode::Clamp,
                                    None,
                                );

                                paint.set_color_source(&gradient);
                                builder.draw_rounded_rect(&rect, &radii, &paint);
                            }
                            BackgroundTag::PatternSlash => {
                                let hsl_color = q.background.solid;
                                let rgba_color = hsl_color.to_rgb();
                                let color = Color::new_srgba(
                                    rgba_color.r,
                                    rgba_color.g,
                                    rgba_color.b,
                                    rgba_color.a,
                                );

                                paint.set_color(color);
                                builder.draw_rounded_rect(&rect, &radii, &paint);
                            }
                        }

                        let has_border = q.border_widths.top.0 > 0.0
                            || q.border_widths.right.0 > 0.0
                            || q.border_widths.bottom.0 > 0.0
                            || q.border_widths.left.0 > 0.0;

                        if has_border {
                            let border_rgba = q.border_color.to_rgb();
                            let border_color = Color::new_srgba(
                                border_rgba.r,
                                border_rgba.g,
                                border_rgba.b,
                                border_rgba.a,
                            );
                            let inner_rect = Rect::new(
                                Point::new(
                                    origin.x.0 + q.border_widths.left.0,
                                    origin.y.0 + q.border_widths.top.0,
                                ),
                                Size::new(
                                    size.width.0 - q.border_widths.left.0 - q.border_widths.right.0,
                                    size.height.0
                                        - q.border_widths.top.0
                                        - q.border_widths.bottom.0,
                                ),
                            );
                            let inner_radii: impellers::RoundingRadii = unsafe {
                                std::mem::transmute([
                                    (q.corner_radii.top_left.0
                                        - q.border_widths.left.0.max(q.border_widths.top.0))
                                    .max(0.0),
                                    (q.corner_radii.top_left.0
                                        - q.border_widths.left.0.max(q.border_widths.top.0))
                                    .max(0.0),
                                    (q.corner_radii.bottom_left.0
                                        - q.border_widths.left.0.max(q.border_widths.bottom.0))
                                    .max(0.0),
                                    (q.corner_radii.bottom_left.0
                                        - q.border_widths.left.0.max(q.border_widths.bottom.0))
                                    .max(0.0),
                                    (q.corner_radii.top_right.0
                                        - q.border_widths.right.0.max(q.border_widths.top.0))
                                    .max(0.0),
                                    (q.corner_radii.top_right.0
                                        - q.border_widths.right.0.max(q.border_widths.top.0))
                                    .max(0.0),
                                    (q.corner_radii.bottom_right.0
                                        - q.border_widths.right.0.max(q.border_widths.bottom.0))
                                    .max(0.0),
                                    (q.corner_radii.bottom_right.0
                                        - q.border_widths.right.0.max(q.border_widths.bottom.0))
                                    .max(0.0),
                                ])
                            };

                            paint.set_color(border_color);
                            builder.draw_rounded_rect_difference(
                                &rect,
                                &radii,
                                &inner_rect,
                                &inner_radii,
                                &paint,
                            );
                        }
                    }
                }
                PrimitiveBatch::Polygons(polygons) => {
                    for polygon in polygons.iter() {
                        if polygon.points.is_empty() {
                            continue;
                        }

                        let mut path_builder = PathBuilder::default();

                        let first_point = &polygon.points[0];
                        path_builder.move_to(Point::new(first_point.x.0, first_point.y.0));

                        for point in polygon.points.iter().skip(1) {
                            path_builder.line_to(Point::new(point.x.0, point.y.0));
                        }

                        path_builder.close();

                        let impeller_path = path_builder.take_path_new(FillType::NonZero);

                        match polygon.background.tag {
                            BackgroundTag::Solid => {
                                let polygon_rgba = polygon.background.solid.to_rgb();
                                let polygon_color = Color::new_srgba(
                                    polygon_rgba.r,
                                    polygon_rgba.g,
                                    polygon_rgba.b,
                                    polygon_rgba.a,
                                );

                                paint.set_color(polygon_color);
                                builder.draw_path(&impeller_path, &paint);
                            }
                            BackgroundTag::LinearGradient => {
                                let origin = polygon.bounds.origin;
                                let size = polygon.bounds.size;

                                let angle = polygon.background.gradient_angle_or_pattern_height;
                                let angle_rad = angle.to_radians();

                                let center_x = origin.x.0 + size.width.0 / 2.0;
                                let center_y = origin.y.0 + size.height.0 / 2.0;

                                let diagonal =
                                    ((size.width.0 * size.width.0 + size.height.0 * size.height.0)
                                        .sqrt())
                                        / 2.0;

                                let start = Point::new(
                                    center_x - angle_rad.sin() * diagonal,
                                    center_y - angle_rad.cos() * diagonal,
                                );
                                let end = Point::new(
                                    center_x + angle_rad.sin() * diagonal,
                                    center_y + angle_rad.cos() * diagonal,
                                );

                                let color0 = polygon.background.colors[0].color.to_rgb();
                                let color1 = polygon.background.colors[1].color.to_rgb();

                                let colors = [
                                    Color::new_srgba(color0.r, color0.g, color0.b, color0.a),
                                    Color::new_srgba(color1.r, color1.g, color1.b, color1.a),
                                ];

                                let stops = [
                                    polygon.background.colors[0].percentage,
                                    polygon.background.colors[1].percentage,
                                ];

                                let gradient = ColorSource::new_linear_gradient(
                                    start,
                                    end,
                                    &colors,
                                    &stops,
                                    TileMode::Clamp,
                                    None,
                                );

                                paint.set_color_source(&gradient);
                                builder.draw_path(&impeller_path, &paint);
                            }
                            BackgroundTag::PatternSlash => {
                                let polygon_rgba = polygon.background.solid.to_rgb();
                                let polygon_color = Color::new_srgba(
                                    polygon_rgba.r,
                                    polygon_rgba.g,
                                    polygon_rgba.b,
                                    polygon_rgba.a,
                                );

                                paint.set_color(polygon_color);
                                builder.draw_path(&impeller_path, &paint);
                            }
                        }

                        if polygon.border_width.0 > 0.0 {
                            let border_rgba = polygon.border_color.to_rgb();
                            let border_color = Color::new_srgba(
                                border_rgba.r,
                                border_rgba.g,
                                border_rgba.b,
                                border_rgba.a,
                            );

                            let mut border_paint = Paint::default();
                            border_paint.set_color(border_color);
                            border_paint.set_stroke_width(polygon.border_width.0);
                            border_paint.set_draw_style(DrawStyle::Stroke);

                            builder.draw_path(&impeller_path, &border_paint);
                        }
                    }
                }
                PrimitiveBatch::Paths(paths) => {
                    for path in paths.iter() {
                        let mut path_builder = PathBuilder::default();

                        if path.vertices.is_empty() {
                            continue;
                        }
                        let origin = path.bounds.origin;
                        let size = path.bounds.size;

                        path_builder.add_rect(&Rect::new(
                            Point::new(origin.x.0, origin.y.0),
                            Size::new(size.width.0, size.height.0),
                        ));

                        let impeller_path = path_builder.take_path_new(FillType::NonZero);
                        let path_color = path.color.solid.to_rgb();
                        paint.set_color(Color::new_srgba(
                            path_color.r,
                            path_color.g,
                            path_color.b,
                            path_color.a,
                        ));

                        builder.draw_path(&impeller_path, &paint);
                    }
                }
                // TODO: Once draw_shadow is available in prebuilt libraries, switch to
                // the native API for better Material Design shadow rendering.
                PrimitiveBatch::Shadows(shadows) => {
                    for shadow in shadows.iter() {
                        let origin = shadow.bounds.origin;
                        let size = shadow.bounds.size;

                        let radii: impellers::RoundingRadii = unsafe {
                            std::mem::transmute([
                                shadow.corner_radii.top_left.0,
                                shadow.corner_radii.top_left.0,
                                shadow.corner_radii.bottom_left.0,
                                shadow.corner_radii.bottom_left.0,
                                shadow.corner_radii.top_right.0,
                                shadow.corner_radii.top_right.0,
                                shadow.corner_radii.bottom_right.0,
                                shadow.corner_radii.bottom_right.0,
                            ])
                        };

                        let blur_sigma = shadow.blur_radius.0 / 2.0;
                        let spread = shadow.blur_radius.0 * 3.0;

                        let shadow_rect = Rect::new(
                            Point::new(origin.x.0 - spread, origin.y.0 - spread),
                            Size::new(size.width.0 + 2.0 * spread, size.height.0 + 2.0 * spread),
                        );

                        let shadow_rgba = shadow.color.to_rgb();
                        let shadow_color = Color::new_srgba(
                            shadow_rgba.r,
                            shadow_rgba.g,
                            shadow_rgba.b,
                            shadow_rgba.a,
                        );

                        {
                            let mut shadow_paint = Paint::default();
                            shadow_paint.set_color(shadow_color);
                            if blur_sigma > 0.0 {
                                let blur_filter =
                                    ImageFilter::new_blur(blur_sigma, blur_sigma, TileMode::Clamp);
                                shadow_paint.set_image_filter(&blur_filter);
                            }
                            builder.draw_rounded_rect(&shadow_rect, &radii, &shadow_paint);
                        }
                    }
                }
                PrimitiveBatch::Underlines(underlines) => {
                    for underline in underlines.iter() {
                        let origin = underline.bounds.origin;
                        let size = underline.bounds.size;

                        let underline_rgba = underline.color.to_rgb();
                        let underline_color = Color::new_srgba(
                            underline_rgba.r,
                            underline_rgba.g,
                            underline_rgba.b,
                            underline_rgba.a,
                        );

                        paint.set_color(underline_color);

                        if underline.wavy != 0 {
                            let y = origin.y.0 + size.height.0 / 2.0;
                            let start = Point::new(origin.x.0, y);
                            let end = Point::new(origin.x.0 + size.width.0, y);

                            let wave_length = underline.thickness.0 * 4.0;
                            builder.draw_dashed_line(
                                start,
                                end,
                                wave_length,
                                wave_length / 2.0,
                                &paint,
                            );
                        } else {
                            let rect = Rect::new(
                                Point::new(origin.x.0, origin.y.0),
                                Size::new(size.width.0, underline.thickness.0),
                            );
                            builder.draw_rect(&rect, &paint);
                        }
                    }
                }

                PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites,
                } => {
                    let texture = self.sprite_atlas.get_texture(texture_id);

                    if let Some(texture) = texture {
                        for sprite in sprites.iter() {
                            let origin = sprite.bounds.origin;
                            let size = sprite.bounds.size;

                            let rgba_color = sprite.color.to_rgb();
                            let color = Color::new_srgba(
                                rgba_color.r,
                                rgba_color.g,
                                rgba_color.b,
                                rgba_color.a,
                            );

                            let mut sprite_paint = Paint::default();
                            let color_filter = ColorFilter::new_blend(color, BlendMode::SourceIn);
                            sprite_paint.set_color_filter(&color_filter);

                            let tile_bounds = sprite.tile.bounds;
                            let src_rect = Rect::new(
                                Point::new(
                                    tile_bounds.origin.x.0 as f32,
                                    tile_bounds.origin.y.0 as f32,
                                ),
                                Size::new(
                                    tile_bounds.size.width.0 as f32,
                                    tile_bounds.size.height.0 as f32,
                                ),
                            );

                            let dst_rect = Rect::new(
                                Point::new(origin.x.0, origin.y.0),
                                Size::new(size.width.0, size.height.0),
                            );

                            let is_identity = sprite.transformation.rotation_scale
                                == [[1.0, 0.0], [0.0, 1.0]]
                                && sprite.transformation.translation == [0.0, 0.0];

                            if !is_identity {
                                builder.save();

                                let transform = sprite.transformation;
                                let matrix = Matrix::new(
                                    transform.rotation_scale[0][0],
                                    transform.rotation_scale[1][0],
                                    0.0,
                                    0.0,
                                    transform.rotation_scale[0][1],
                                    transform.rotation_scale[1][1],
                                    0.0,
                                    0.0,
                                    0.0,
                                    0.0,
                                    1.0,
                                    0.0,
                                    transform.translation[0],
                                    transform.translation[1],
                                    0.0,
                                    1.0,
                                );

                                builder.transform(&matrix);
                            }

                            builder.draw_texture_rect(
                                &texture,
                                &src_rect,
                                &dst_rect,
                                TextureSampling::Linear,
                                Some(&sprite_paint),
                            );

                            if !is_identity {
                                builder.restore();
                            }
                        }
                    } else {
                        // Fallback: draw colored rectangles when texture is not available
                        for sprite in sprites.iter() {
                            let origin = sprite.bounds.origin;
                            let size = sprite.bounds.size;

                            let rgba_color = sprite.color.to_rgb();
                            let color = Color::new_srgba(
                                rgba_color.r,
                                rgba_color.g,
                                rgba_color.b,
                                rgba_color.a,
                            );

                            paint.set_color(color);

                            let rect = Rect::new(
                                Point::new(origin.x.0, origin.y.0),
                                Size::new(size.width.0, size.height.0),
                            );

                            builder.draw_rect(&rect, &paint);
                        }
                    }
                }
                PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites,
                } => {
                    let texture = self.sprite_atlas.get_texture(texture_id);

                    if let Some(texture) = texture {
                        for sprite in sprites.iter() {
                            let origin = sprite.bounds.origin;
                            let size = sprite.bounds.size;

                            let mut sprite_paint = Paint::default();
                            sprite_paint.set_blend_mode(BlendMode::SourceOver);

                            if sprite.grayscale {
                                let grayscale_matrix = ColorMatrix {
                                    m: [
                                        0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.2126, 0.7152, 0.0722,
                                        0.0, 0.0, 0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.0, 0.0, 0.0,
                                        1.0, 0.0,
                                    ],
                                };
                                let filter = ColorFilter::new_matrix(grayscale_matrix);
                                sprite_paint.set_color_filter(&filter);
                            }

                            let tile_bounds = sprite.tile.bounds;
                            let src_rect = Rect::new(
                                Point::new(
                                    tile_bounds.origin.x.0 as f32,
                                    tile_bounds.origin.y.0 as f32,
                                ),
                                Size::new(
                                    tile_bounds.size.width.0 as f32,
                                    tile_bounds.size.height.0 as f32,
                                ),
                            );

                            let dst_rect = Rect::new(
                                Point::new(origin.x.0, origin.y.0),
                                Size::new(size.width.0, size.height.0),
                            );

                            let radii: impellers::RoundingRadii = unsafe {
                                std::mem::transmute([
                                    sprite.corner_radii.top_left.0,
                                    sprite.corner_radii.top_left.0,
                                    sprite.corner_radii.bottom_left.0,
                                    sprite.corner_radii.bottom_left.0,
                                    sprite.corner_radii.top_right.0,
                                    sprite.corner_radii.top_right.0,
                                    sprite.corner_radii.bottom_right.0,
                                    sprite.corner_radii.bottom_right.0,
                                ])
                            };

                            let has_radii = sprite.corner_radii.top_left.0 > 0.0
                                || sprite.corner_radii.top_right.0 > 0.0
                                || sprite.corner_radii.bottom_left.0 > 0.0
                                || sprite.corner_radii.bottom_right.0 > 0.0;

                            let content_mask_bounds = sprite.content_mask.bounds;
                            let content_mask_rect = Rect::new(
                                Point::new(
                                    content_mask_bounds.origin.x.0,
                                    content_mask_bounds.origin.y.0,
                                ),
                                Size::new(
                                    content_mask_bounds.size.width.0,
                                    content_mask_bounds.size.height.0,
                                ),
                            );

                            builder.save();

                            let mut path_builder = PathBuilder::default();
                            path_builder.add_rect(&content_mask_rect);
                            let content_mask_path = path_builder.take_path_new(FillType::NonZero);
                            builder.clip_path(&content_mask_path, ClipOperation::Intersect);

                            if has_radii {
                                let mut path_builder = PathBuilder::default();
                                path_builder.add_rounded_rect(&dst_rect, &radii);
                                let clip_path = path_builder.take_path_new(FillType::NonZero);
                                builder.clip_path(&clip_path, ClipOperation::Intersect);
                            }

                            builder.draw_texture_rect(
                                &texture,
                                &src_rect,
                                &dst_rect,
                                TextureSampling::Linear,
                                Some(&sprite_paint),
                            );

                            builder.restore();
                        }
                    } else {
                        // Fallback: draw colored rounded rectangles when texture is not available
                        for sprite in sprites.iter() {
                            let origin = sprite.bounds.origin;
                            let size = sprite.bounds.size;

                            let color = Color::new_srgba(1.0, 1.0, 1.0, sprite.opacity);
                            paint.set_color(color);

                            let radii: impellers::RoundingRadii = unsafe {
                                std::mem::transmute([
                                    sprite.corner_radii.top_left.0,
                                    sprite.corner_radii.top_left.0,
                                    sprite.corner_radii.bottom_left.0,
                                    sprite.corner_radii.bottom_left.0,
                                    sprite.corner_radii.top_right.0,
                                    sprite.corner_radii.top_right.0,
                                    sprite.corner_radii.bottom_right.0,
                                    sprite.corner_radii.bottom_right.0,
                                ])
                            };

                            let rect = Rect::new(
                                Point::new(origin.x.0, origin.y.0),
                                Size::new(size.width.0, size.height.0),
                            );

                            builder.draw_rounded_rect(&rect, &radii, &paint);
                        }
                    }
                }
                PrimitiveBatch::Surfaces(_paint_surfaces) => {}
            }
        }
        if self.framebuffer.is_none() {
            return;
        }
        self.framebuffer
            .as_mut()
            .expect("Didn't have framebuffer while drawing.")
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
        // Ensure the context is current before resizing
        if !self.gl_context.is_current() {
            self.gl_context
                .make_current(&self.gl_surface)
                .expect("Failed to make GL context current");
        }

        self.gl_surface.resize(
            &self.gl_context,
            NonZeroU32::new((size.width.0 as u32).max(1)).unwrap(),
            NonZeroU32::new((size.height.0 as u32).max(1)).unwrap(),
        );
        self.framebuffer = unsafe {
            self.impeller_context.wrap_fbo(
                0,
                impellers::PixelFormat::RGBA8888,
                ISize::new((size.width.0 as u32).max(1) as i64, (size.height.0 as u32).max(1) as i64),
            )
        };
        log::debug!("Updated drawable size: {:?}", size);
    }
    fn update_transparency(&mut self, transparent: bool) {
        // Note: The surface is kept transparent at the GL level (alpha_size: 8).
        // We emulate transparency by drawing either a transparent or opaque background
        // in the draw method, avoiding the need to recreate the GL surface.
        self.transparent = transparent;
    }

    fn destroy(&mut self) {
        self.framebuffer = None;
    }
}





