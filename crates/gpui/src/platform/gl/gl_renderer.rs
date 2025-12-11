use crate::{PlatformRenderer, platform::gl::gl_atlas::GlAtlas};
use glow::HasContext;
use khronos_egl as egl;
#[cfg(target_os = "linux")]
use raw_window_handle::RawDisplayHandle;
use raw_window_handle::RawWindowHandle;
use wayland_egl::WlEglSurface;

pub struct GlRenderer {
    atlas: std::sync::Arc<GlAtlas>,
    gl: glow::Context,
    egl: egl::Instance<egl::Static>,
    egl_context: egl::Context,
    egl_display: egl::Display,
    egl_surface: egl::Surface,
    wl_egl_surface: WlEglSurface,
}

impl GlRenderer {
    pub fn new<I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle>(
        window: &I,
        surface_config: crate::SurfaceConfig,
    ) -> anyhow::Result<Self> {
        let egl = egl::Instance::new(egl::Static);

        let display_handle = if let Ok(display_handle) = window.display_handle() {
            display_handle.as_raw()
        } else {
            return Err(anyhow::anyhow!("Could not get display handle"));
        };

        let native_display = match display_handle {
            RawDisplayHandle::Wayland(handle) => handle.display.as_ptr(),
            _ => return Err(anyhow::anyhow!("Unsupported display handle")),
        };
        let egl_display = unsafe { egl.get_display(native_display) }
            .ok_or(anyhow::anyhow!("Failed to get EGL display"))?;

        let (major, minor) = egl.initialize(egl_display)?;
        log::info!("EGL version: {}.{}", major, minor);

        // Choose config
        let config_attribs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES2_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::DEPTH_SIZE,
            24,
            egl::NONE,
        ];

        let config = egl
            .choose_first_config(egl_display, &config_attribs)?
            .ok_or(anyhow::anyhow!(
                "Couldn't find suitable config for the window!"
            ))?;

        // Get native window handle
        let window_handle = if let Ok(window_handle) = window.window_handle() {
            window_handle.as_raw()
        } else {
            return Err(anyhow::anyhow!("Could not get window handle"));
        };
        let wl_surface = match window_handle {
            RawWindowHandle::Wayland(handle) => handle.surface.as_ptr(),
            _ => return Err(anyhow::anyhow!("Expected Wayland window")),
        };

        // Create wl_egl_window - this is required for Wayland
        // SAFETY: wl_surface pointer is valid and we keep wl_egl_surface alive
        let wl_egl_surface = unsafe {
            WlEglSurface::new_from_raw(
                wl_surface as *mut _,
                surface_config.width as i32,
                surface_config.height as i32,
            )?
        };

        // Create EGL window surface using the wl_egl_window pointer
        let egl_surface = unsafe {
            egl.create_window_surface(
                egl_display,
                config,
                wl_egl_surface.ptr() as egl::NativeWindowType,
                None,
            )?
        };

        // Create OpenGL ES 3.0 context
        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            2,
            egl::CONTEXT_MINOR_VERSION,
            0,
            egl::NONE,
        ];

        egl.bind_api(egl::OPENGL_ES_API)?;
        let egl_context = egl.create_context(egl_display, config, None, &context_attribs)?;

        // Make context current
        egl.make_current(
            egl_display,
            Some(egl_surface),
            Some(egl_surface),
            Some(egl_context),
        )?;

        // Create glow context
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                egl.get_proc_address(s)
                    .map(|p| p as *const _)
                    .unwrap_or(std::ptr::null())
            })
        };

        unsafe {
            let renderer = gl.get_parameter_string(glow::RENDERER);
            let version = gl.get_parameter_string(glow::VERSION);
            println!("Renderer: {}", renderer);
            println!("OpenGL ES version: {}", version);
        }

        Ok(Self {
            atlas: std::sync::Arc::new(GlAtlas {}),
            egl,
            egl_context,
            egl_display,
            egl_surface,
            wl_egl_surface,
            gl,
        })
    }
}

impl PlatformRenderer for GlRenderer {
    fn draw(&mut self, scene: &crate::Scene) {
        unsafe {
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        self.egl
            .swap_buffers(self.egl_display, self.egl_surface)
            .expect("swap_buffers failed");
    }

    fn sprite_atlas(&self) -> std::sync::Arc<dyn crate::PlatformAtlas> {
        self.atlas.clone()
    }

    fn gpu_specs(&self) -> crate::GpuSpecs {
        todo!()
    }

    fn update_drawable_size(&mut self, size: crate::Size<crate::DevicePixels>) {
        self.wl_egl_surface
            .resize(size.width.0 as i32, size.height.0 as i32, 0, 0);
        unsafe {
            self.gl
                .viewport(0, 0, size.width.0 as i32, size.height.0 as i32);
        }
    }

    fn update_transparency(&mut self, transparent: bool) {
        println!("TODO: update transparancy to {}", transparent);
    }

    fn destroy(&mut self) {
        let _ = self.egl.make_current(self.egl_display, None, None, None);
        let _ = self.egl.destroy_surface(self.egl_display, self.egl_surface);
        let _ = self.egl.destroy_context(self.egl_display, self.egl_context);
        let _ = self.egl.terminate(self.egl_display);
        // wl_egl_surface drops here after EGL cleanup
    }

    fn viewport_size(&self) -> crate::Size<f32> {
        todo!()
    }
}
