mod dispatcher;
mod headless;
mod keyboard;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

pub(crate) use dispatcher::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

#[cfg(all(feature = "screen-capture", any(feature = "wayland", feature = "x11")))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(all(feature = "screen-capture", any(feature = "wayland", feature = "x11"))))]
pub(crate) type PlatformScreenCaptureFrame = ();

// Renderer type - Impeller if feature is enabled, otherwise Blade
#[cfg(all(any(feature = "wayland", feature = "x11"), feature = "linux-impeller"))]
pub(crate) type Renderer = crate::platform::impeller::ImpellerRenderer;
#[cfg(all(
    any(feature = "wayland", feature = "x11"),
    not(feature = "linux-impeller")
))]
pub(crate) type Renderer = crate::platform::blade::BladeRenderer;

// Context type for renderer initialization
#[cfg(all(any(feature = "wayland", feature = "x11"), feature = "linux-impeller"))]
pub(crate) type RendererContext = crate::platform::impeller::ImpellerContext;
#[cfg(all(
    any(feature = "wayland", feature = "x11"),
    not(feature = "linux-impeller")
))]
pub(crate) type RendererContext = crate::platform::blade::BladeContext;

// Renderer configuration parameters type
#[cfg(all(any(feature = "wayland", feature = "x11"), feature = "linux-impeller"))]
pub(crate) type RendererParams = (u32, u32);
#[cfg(all(
    any(feature = "wayland", feature = "x11"),
    not(feature = "linux-impeller")
))]
pub(crate) type RendererParams = crate::platform::blade::BladeSurfaceConfig;

#[cfg(feature = "wayland")]
pub use wayland::layer_shell;
