use std::{ffi::CString, num::NonZeroU32};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, Version},
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext as _},
    surface::{GlSurface, SurfaceAttributesBuilder, WindowSurface},
};
use impellers::{
    BlendMode, ClipOperation, Color, ColorFilter, ColorMatrix, DisplayListBuilder, FillType, ISize,
    ImageFilter, Matrix, Paint, PathBuilder, Point, Rect, Size, TextureSampling, TileMode,
};

use crate::{
    GpuSpecs, PlatformRenderer, PrimitiveBatch,
    platform::impeller::{ImpellerAtlas, ImpellerContext},
};

pub struct ImpellerRenderer {
    sprite_atlas: std::sync::Arc<ImpellerAtlas>,
    framebuffer: Option<impellers::Surface>,
    gl_surface: glutin::surface::Surface<WindowSurface>,
    // TODO: Maybe move this to ImpellerContext
    gl_context: glutin::context::PossiblyCurrentContext,
    impeller_context: impellers::Context,
    glow_context: glow::Context,
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
            NonZeroU32::new(config.0).unwrap(),
            NonZeroU32::new(config.1).unwrap(),
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
                .wrap_fbo(0, impellers::PixelFormat::RGBA8888, ISize::new(0, 0))
                .unwrap()
        };

        Ok(Self {
            sprite_atlas,
            glow_context,
            gl_context,
            impeller_context,
            gl_surface,
            framebuffer: Some(framebuffer),
        })
    }
}
impl PlatformRenderer for ImpellerRenderer {
    type RenderParams = (u32, u32);

    fn draw(&mut self, scene: &crate::Scene) {
        let mut builder = DisplayListBuilder::new(None);
        let mut paint = Paint::default();
        paint.set_color(Color::BLACKBERRY);
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
                            let border_width = q
                                .border_widths
                                .top
                                .0
                                .max(q.border_widths.right.0)
                                .max(q.border_widths.bottom.0)
                                .max(q.border_widths.left.0);
                            let inner_rect = Rect::new(
                                Point::new(origin.x.0 + border_width, origin.y.0 + border_width),
                                Size::new(
                                    size.width.0 - 2.0 * border_width,
                                    size.height.0 - 2.0 * border_width,
                                ),
                            );
                            let inner_radii: impellers::RoundingRadii = unsafe {
                                std::mem::transmute([
                                    (q.corner_radii.top_left.0 - border_width).max(0.0),
                                    (q.corner_radii.top_left.0 - border_width).max(0.0),
                                    (q.corner_radii.bottom_left.0 - border_width).max(0.0),
                                    (q.corner_radii.bottom_left.0 - border_width).max(0.0),
                                    (q.corner_radii.top_right.0 - border_width).max(0.0),
                                    (q.corner_radii.top_right.0 - border_width).max(0.0),
                                    (q.corner_radii.bottom_right.0 - border_width).max(0.0),
                                    (q.corner_radii.bottom_right.0 - border_width).max(0.0),
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
                                    transform.translation[0],
                                    transform.rotation_scale[0][1],
                                    transform.rotation_scale[1][1],
                                    0.0,
                                    transform.translation[1],
                                    0.0,
                                    0.0,
                                    1.0,
                                    0.0,
                                    0.0,
                                    0.0,
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

                            let color = Color::new_srgba(1.0, 1.0, 1.0, sprite.opacity);
                            paint.set_color(color);

                            if sprite.grayscale {
                                let grayscale_matrix = ColorMatrix {
                                    m: [
                                        0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.2126, 0.7152, 0.0722,
                                        0.0, 0.0, 0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.0, 0.0, 0.0,
                                        1.0, 0.0,
                                    ],
                                };
                                let filter = ColorFilter::new_matrix(grayscale_matrix);
                                paint.set_color_filter(&filter);
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

                            let has_radii = sprite.corner_radii.top_left.0 > 0.0
                                || sprite.corner_radii.top_right.0 > 0.0
                                || sprite.corner_radii.bottom_left.0 > 0.0
                                || sprite.corner_radii.bottom_right.0 > 0.0;

                            if has_radii {
                                builder.save();

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
                                Some(&paint),
                            );

                            if has_radii {
                                builder.restore();
                            }
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
        self.gl_surface.resize(
            &self.gl_context,
            NonZeroU32::new(size.width.0 as u32).unwrap(),
            NonZeroU32::new(size.height.0 as u32).unwrap(),
        );
        self.framebuffer = unsafe {
            self.impeller_context.wrap_fbo(
                0,
                impellers::PixelFormat::RGBA8888,
                ISize::new(size.width.0 as i64, size.height.0 as i64),
            )
        };
        println!("Updated drawable size: {:?}", size);
    }

    fn update_transparency(&mut self, transparent: bool) {
        println!("Transparancy update: {}", transparent);
    }

    fn destroy(&mut self) {
        todo!()
    }
}
