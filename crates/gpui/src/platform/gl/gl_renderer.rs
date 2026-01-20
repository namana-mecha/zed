use std::ffi::CString;
use std::num::NonZeroU32;
use std::sync::Arc;

use glow::HasContext;
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version,
};
use glutin::display::{Display, DisplayApiPreference, GlDisplay};
use glutin::prelude::{GlSurface, PossiblyCurrentGlContext};
use glutin::surface::{
    Surface as GlutinSurface, SurfaceAttributesBuilder, SwapInterval, WindowSurface,
};

use crate::{
    DevicePixels, GpuSpecs, Scene, Size, platform::gl::GlAtlas,
};

pub struct GlSurfaceConfig {
    pub width: u32,
    pub height: u32,
    pub transparent: bool,
}

pub struct GlRenderer {
    gl: Arc<glow::Context>,
    atlas: Arc<GlAtlas>,
    
    viewport_size: Size<DevicePixels>,

    _context: PossiblyCurrentContext,
    _surface: GlutinSurface<WindowSurface>,
    
    transparent: bool,
}

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

        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8).with_depth_size(8).build();

        let gl_config = unsafe { display.find_configs(template) }
            .map_err(|e| anyhow::anyhow!("Failed to discover OpenGL configs: {}", e))?
            .find(|c| c.depth_size() == 24)
            .ok_or_else(|| anyhow::anyhow!("Couldn't find suitable 24-bit depth config!"))?;

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

        surface
            .set_swap_interval(&context, SwapInterval::DontWait)
            .map_err(|e| anyhow::anyhow!("Failed to set swap interval: {}", e))?;

        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                let s = CString::new(s).unwrap();
                display.get_proc_address(&s)
            })
        };
        let gl = Arc::new(gl);
        let atlas = Arc::new(GlAtlas::new());

        Ok(Self {
            gl,
            atlas,
            viewport_size: Size { width: DevicePixels(config.width as i32), height: DevicePixels(config.height as i32) },
            _context: context,
            _surface: surface,
            transparent: config.transparent,
        })
    }

    pub fn draw(&mut self, _scene: &Scene) {
        if self.viewport_size.width.0 <= 0 || self.viewport_size.height.0 <= 0 { return; }
        if let Err(e) = self._context.make_current(&self._surface) { log::error!("Make current failed: {}", e); return; }
        
        self.atlas.before_frame(&self.gl);

        unsafe {
            self.gl.viewport(0, 0, self.viewport_size.width.0, self.viewport_size.height.0);
            self.gl.clear_color(0.0, 0.0, 0.0, if self.transparent { 0.0 } else { 1.0 });
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        // TODO: Add rendering logic here

        if let Err(e) = self._surface.swap_buffers(&self._context) { log::error!("Swap buffers failed: {}", e); }
    }

    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        self.viewport_size = size;
        let width = NonZeroU32::new(size.width.0 as u32).unwrap_or(NonZeroU32::MIN);
        let height = NonZeroU32::new(size.height.0 as u32).unwrap_or(NonZeroU32::MIN);
        self._surface.resize(&self._context, width, height);
    }
    
    pub fn update_transparency(&mut self, transparency: bool) { self.transparent = transparency; }
    
    pub fn sprite_atlas(&self) -> &std::sync::Arc<GlAtlas> { &self.atlas }
    
    pub fn gpu_specs(&self) -> GpuSpecs {
        let renderer = unsafe { self.gl.get_parameter_string(glow::RENDERER) };
        let version = unsafe { self.gl.get_parameter_string(glow::VERSION) };
        GpuSpecs { is_software_emulated: false, device_name: renderer, driver_name: "OpenGL".to_string(), driver_info: version }
    }
    
    pub fn destroy(&mut self) {
        let _ = self._context.make_current(&self._surface);
        // Clean up GL resources here
    }
}

impl crate::platform::PlatformRenderer for GlRenderer {
    type RenderParams = GlSurfaceConfig;
    fn draw(&mut self, scene: &Scene) { self.draw(scene) }
    fn sprite_atlas(&self) -> Arc<dyn crate::platform::PlatformAtlas> { self.atlas.clone() }
    fn gpu_specs(&self) -> GpuSpecs { self.gpu_specs() }
    fn update_drawable_size(&mut self, size: Size<DevicePixels>) { self.update_drawable_size(size) }
    fn update_transparency(&mut self, transparent: bool) { self.update_transparency(transparent) }
    fn destroy(&mut self) { self.destroy() }
}
