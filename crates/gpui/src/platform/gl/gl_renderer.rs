use crate::{Bounds, PlatformRenderer, ScaledPixels, platform::gl::gl_atlas::GlAtlas};
use femtovg::{Canvas, Color, renderer::OpenGl};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version,
    },
    display::{GetGlDisplay, GlDisplay},
    prelude::*,
    surface::{Surface as GlutinSurface, SurfaceAttributesBuilder, WindowSurface},
};
use std::num::NonZeroU32;

mod monochrome_sprite;
mod quad;

pub struct GlRenderer {
    atlas: std::sync::Arc<GlAtlas>,
    pub canvas: Canvas<OpenGl>,
    pub surface: GlutinSurface<WindowSurface>,
    pub context: PossiblyCurrentContext,
    previous_bounds: Option<Bounds<ScaledPixels>>,
}

impl GlRenderer {
    pub fn new<I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle>(
        window: &I,
        surface_config: crate::SurfaceConfig,
    ) -> anyhow::Result<Self> {
        let display_handle = window
            .display_handle()
            .expect("Couldn't get display handle")
            .as_raw();

        let display = unsafe {
            glutin::display::Display::new(
                display_handle,
                glutin::display::DisplayApiPreference::Egl,
            )?
        };

        let template = ConfigTemplateBuilder::new().with_alpha_size(8);

        let config = unsafe { display.find_configs(template.build())? }
            .find(|c| c.depth_size() == 24)
            .ok_or_else(|| anyhow::anyhow!("Couldn't find suitable 24-bit depth config!"))?;

        let window_handle = window
            .window_handle()
            .expect("Couldn't get window handle")
            .as_raw();

        let width = NonZeroU32::new(surface_config.width as u32).unwrap_or(NonZeroU32::MIN);
        let height = NonZeroU32::new(surface_config.height as u32).unwrap_or(NonZeroU32::MIN);

        let surface_attributes =
            SurfaceAttributesBuilder::<WindowSurface>::new().build(window_handle, width, height);

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
        let gl_display = config.display();

        let not_current_context = unsafe {
            gl_display
                .create_context(&config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_display
                        .create_context(&config, &fallback_context_attributes)
                        .unwrap_or_else(|_| {
                            gl_display
                                .create_context(&config, &legacy_context_attributes)
                                .expect("Unable to create GL context")
                        })
                })
        };

        let surface = unsafe { gl_display.create_window_surface(&config, &surface_attributes)? };
        let context = not_current_context.make_current(&surface)?;

        let renderer =
            unsafe { OpenGl::new_from_function_cstr(|s| gl_display.get_proc_address(s).cast()) }
                .expect("Cannot create renderer");
        let mut canvas = Canvas::new(renderer).expect("Cannot create canvas");
        canvas.set_size(width.into(), height.into(), 1.0);

        Ok(Self {
            atlas: std::sync::Arc::new(GlAtlas::new()),
            canvas,
            surface,
            context,
            previous_bounds: None,
        })
    }
}

impl PlatformRenderer for GlRenderer {
    fn draw(&mut self, scene: &crate::Scene) {
        self.context
            .make_current(&self.surface)
            .expect("Couldn't make current");
        self.canvas.clear_rect(0, 0, 100, 100, Color::white());
        for batch in scene.batches() {
            match batch {
                crate::PrimitiveBatch::Shadows(shadows) => {}
                crate::PrimitiveBatch::Quads(quads) => self.draw_quads(quads),
                crate::PrimitiveBatch::Paths(paths) => todo!(),
                crate::PrimitiveBatch::Underlines(underlines) => todo!(),
                crate::PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    sprites,
                } => self.draw_monochrome_sprites(sprites),
                crate::PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    sprites,
                } => todo!(),
                crate::PrimitiveBatch::Surfaces(paint_surfaces) => todo!(),
            }
        }
        self.canvas.flush();
        self.surface
            .swap_buffers(&self.context)
            .expect("Couldn't swap buffers!");
    }

    fn sprite_atlas(&self) -> std::sync::Arc<dyn crate::PlatformAtlas> {
        self.atlas.clone()
    }

    fn gpu_specs(&self) -> crate::GpuSpecs {
        todo!()
    }

    fn update_drawable_size(&mut self, size: crate::Size<crate::DevicePixels>) {
        let width = NonZeroU32::new(size.width.0 as u32).unwrap_or(NonZeroU32::MIN);
        let height = NonZeroU32::new(size.height.0 as u32).unwrap_or(NonZeroU32::MIN);
        self.surface.resize(&self.context, width, height);
        self.canvas.set_size(width.into(), height.into(), 1.0);
    }

    fn update_transparency(&mut self, transparent: bool) {
        println!("TODO: update transparancy to {}", transparent);
    }

    fn destroy(&mut self) {}

    fn viewport_size(&self) -> crate::Size<f32> {
        todo!()
    }
}
