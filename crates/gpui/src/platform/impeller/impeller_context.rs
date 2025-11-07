use crate::{PlatformRendererContext, platform::impeller::ImpellerRenderer};

pub struct ImpellerContext {}

impl ImpellerContext {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

impl PlatformRendererContext for ImpellerContext {
    type Renderer = ImpellerRenderer;

    fn create_renderer<
        I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle,
    >(
        &self,
        window: &I,
        params: <Self::Renderer as crate::PlatformRenderer>::RenderParams,
    ) -> anyhow::Result<Self::Renderer> {
        Self::Renderer::new(self, window, params)
    }
}
