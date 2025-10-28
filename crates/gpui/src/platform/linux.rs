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

// Renderer type - currently using Blade, can be swapped with GLES2 or other renderers
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) type Renderer = crate::platform::blade::BladeRenderer;

// Context type for renderer initialization
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) type RendererContext = crate::platform::blade::BladeContext;

// Renderer configuration parameters type
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) type RendererParams = crate::platform::blade::BladeSurfaceConfig;
