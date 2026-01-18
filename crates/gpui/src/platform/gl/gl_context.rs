use super::GlRenderer;
use crate::platform::{PlatformRenderer, PlatformRendererContext};

pub struct GlContext {}

impl GlContext {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

impl PlatformRendererContext for GlContext {
    type Renderer = GlRenderer;

    fn create_renderer<
        I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle,
    >(
        &self,
        window: &I,
        params: <Self::Renderer as PlatformRenderer>::RenderParams,
    ) -> anyhow::Result<Self::Renderer> {
        Self::Renderer::new(params, window)
    }
}
