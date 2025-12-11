use crate::{PlatformRendererContext, platform::gl::GlRenderer};

pub struct GlContext;
impl PlatformRendererContext for GlContext {
    type Renderer = GlRenderer;

    fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    fn create_renderer<
        I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle,
    >(
        &self,
        window: &I,
        config: crate::SurfaceConfig,
    ) -> anyhow::Result<Self::Renderer> {
        GlRenderer::new(window, config)
    }
}
