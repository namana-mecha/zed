use std::{ffi::CString, num::NonZeroU32};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, Version},
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext as _},
    surface::{GlSurface, SurfaceAttributesBuilder, WindowSurface},
};
use impellers::{Color, DisplayListBuilder, ISize, Paint, Point, Rect, Size};

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
        Ok(Self {
            sprite_atlas: std::sync::Arc::new(ImpellerAtlas::new()),
            glow_context,
            gl_context,
            impeller_context,
            gl_surface,
            framebuffer: None,
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

                        // Convert corner radii from GPUI's Corners to impellers' RoundingRadii
                        // Using transmute to construct from array since ImpellerPoint is not publicly exported
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

                        // Draw background
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

                        // Draw border if border widths are non-zero
                        let has_border = q.border_widths.top.0 > 0.0
                            || q.border_widths.right.0 > 0.0
                            || q.border_widths.bottom.0 > 0.0
                            || q.border_widths.left.0 > 0.0;

                        if has_border {
                            // Convert border color
                            let border_rgba = q.border_color.to_rgb();
                            let border_color = Color::new_srgba(
                                border_rgba.r,
                                border_rgba.g,
                                border_rgba.b,
                                border_rgba.a,
                            );

                            // For uniform borders, we can use the difference of two rounded rects
                            // This creates a border effect
                            let border_width = q
                                .border_widths
                                .top
                                .0
                                .max(q.border_widths.right.0)
                                .max(q.border_widths.bottom.0)
                                .max(q.border_widths.left.0);

                            // Calculate inner rect by shrinking the outer rect by border width
                            let inner_rect = Rect::new(
                                Point::new(origin.x.0 + border_width, origin.y.0 + border_width),
                                Size::new(
                                    size.width.0 - 2.0 * border_width,
                                    size.height.0 - 2.0 * border_width,
                                ),
                            );

                            // Adjust inner corner radii by subtracting border width
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
                PrimitiveBatch::Paths(paths) => {}
                PrimitiveBatch::Shadows(shadows) => {}
                PrimitiveBatch::Underlines(underlines) => {}
                PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites,
                } => {}
                PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites,
                } => {}
                PrimitiveBatch::Surfaces(paint_surfaces) => {}
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
