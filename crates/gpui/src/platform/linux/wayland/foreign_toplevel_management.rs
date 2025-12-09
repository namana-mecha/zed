use std::sync::{Arc, Mutex};
use thiserror::Error;
use wayland_backend::client::ObjectId;
use wayland_client::Proxy;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1;

use crate::SharedString;

bitflags::bitflags! {
    /// Represents the state of a toplevel window.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct ToplevelState: u32 {
        /// The window is maximized.
        const MAXIMIZED = 1 << 0;
        /// The window is minimized.
        const MINIMIZED = 1 << 1;
        /// The window is activated (has focus).
        const ACTIVATED = 1 << 2;
        /// The window is fullscreen.
        const FULLSCREEN = 1 << 3;
    }
}

impl From<zwlr_foreign_toplevel_handle_v1::State> for ToplevelState {
    fn from(state: zwlr_foreign_toplevel_handle_v1::State) -> Self {
        match state {
            zwlr_foreign_toplevel_handle_v1::State::Maximized => ToplevelState::MAXIMIZED,
            zwlr_foreign_toplevel_handle_v1::State::Minimized => ToplevelState::MINIMIZED,
            zwlr_foreign_toplevel_handle_v1::State::Activated => ToplevelState::ACTIVATED,
            zwlr_foreign_toplevel_handle_v1::State::Fullscreen => ToplevelState::FULLSCREEN,
            _ => ToplevelState::empty(),
        }
    }
}

/// Information about a toplevel window.
#[derive(Clone, Debug, Default)]
pub struct ToplevelInfo {
    /// The window title.
    pub title: Option<SharedString>,
    /// The application ID.
    pub app_id: Option<SharedString>,
    /// The current state of the window.
    pub state: ToplevelState,
    /// The outputs the window is visible on.
    pub outputs: Vec<ObjectId>,
    /// The parent window, if any.
    pub parent: Option<ObjectId>,
}

/// A handle to a foreign toplevel window that can be used to control it.
#[derive(Clone, Debug)]
pub struct ForeignToplevelHandle {
    handle: Arc<zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1>,
    info: Arc<Mutex<ToplevelInfo>>,
}

impl ForeignToplevelHandle {
    pub(crate) fn new(
        handle: zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    ) -> Self {
        Self {
            handle: Arc::new(handle),
            info: Arc::new(Mutex::new(ToplevelInfo::default())),
        }
    }

    /// Returns the complete information about this toplevel window.
    pub fn info(&self) -> ToplevelInfo {
        self.info.lock().unwrap().clone()
    }

    /// Returns the window title, if available.
    pub fn title(&self) -> Option<SharedString> {
        self.info.lock().unwrap().title.clone()
    }

    /// Returns the application ID, if available.
    pub fn app_id(&self) -> Option<SharedString> {
        self.info.lock().unwrap().app_id.clone()
    }

    /// Returns the current state of the window.
    pub fn state(&self) -> ToplevelState {
        self.info.lock().unwrap().state
    }

    /// Returns whether the window is maximized.
    pub fn is_maximized(&self) -> bool {
        self.state().contains(ToplevelState::MAXIMIZED)
    }

    /// Returns whether the window is minimized.
    pub fn is_minimized(&self) -> bool {
        self.state().contains(ToplevelState::MINIMIZED)
    }

    /// Returns whether the window is activated (has focus).
    pub fn is_activated(&self) -> bool {
        self.state().contains(ToplevelState::ACTIVATED)
    }

    /// Returns whether the window is fullscreen.
    pub fn is_fullscreen(&self) -> bool {
        self.state().contains(ToplevelState::FULLSCREEN)
    }

    /// Requests the compositor to maximize the window.
    pub fn set_maximized(&self) {
        self.handle.set_maximized();
    }

    /// Requests the compositor to unmaximize the window.
    pub fn unset_maximized(&self) {
        self.handle.unset_maximized();
    }

    /// Requests the compositor to minimize the window.
    pub fn set_minimized(&self) {
        self.handle.set_minimized();
    }

    /// Requests the compositor to unminimize the window.
    pub fn unset_minimized(&self) {
        self.handle.unset_minimized();
    }

    /// Requests the compositor to activate the window on the given seat.
    pub fn activate(&self, seat: &wayland_client::protocol::wl_seat::WlSeat) {
        self.handle.activate(seat);
    }

    /// Requests the application to close the window.
    pub fn close(&self) {
        self.handle.close();
    }

    /// Requests the compositor to make the window fullscreen on the given output.
    pub fn set_fullscreen(&self, output: Option<&WlOutput>) {
        self.handle.set_fullscreen(output);
    }

    /// Requests the compositor to exit fullscreen mode.
    pub fn unset_fullscreen(&self) {
        self.handle.unset_fullscreen();
    }

    /// Sets the rectangle on a surface where the toplevel is represented.
    /// This is useful for taskbars and docks.
    pub fn set_rectangle(
        &self,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        self.handle.set_rectangle(surface, x, y, width, height);
    }

    pub(crate) fn handle(&self) -> &zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1 {
        &self.handle
    }

    pub(crate) fn update_title(&self, title: String) {
        self.info.lock().unwrap().title = Some(title.into());
    }

    pub(crate) fn update_app_id(&self, app_id: String) {
        self.info.lock().unwrap().app_id = Some(app_id.into());
    }

    pub(crate) fn update_state(&self, state: Vec<u8>) {
        let mut new_state = ToplevelState::empty();

        for chunk in state.chunks(4) {
            if chunk.len() == 4 {
                let value = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if let Ok(state_enum) = zwlr_foreign_toplevel_handle_v1::State::try_from(value) {
                    new_state |= ToplevelState::from(state_enum);
                }
            }
        }

        self.info.lock().unwrap().state = new_state;
    }

    pub(crate) fn add_output(&self, output: &WlOutput) {
        self.info.lock().unwrap().outputs.push(output.id());
    }

    pub(crate) fn remove_output(&self, output: &WlOutput) {
        let mut info = self.info.lock().unwrap();
        info.outputs.retain(|id| id != &output.id());
    }

    pub(crate) fn update_parent(
        &self,
        parent: Option<&zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1>,
    ) {
        self.info.lock().unwrap().parent = parent.map(|p| p.id());
    }
}

/// Callback type for handling new toplevel windows.
pub type ToplevelCallback = Box<dyn FnMut(ForeignToplevelHandle) + Send>;

/// Manager for foreign toplevel windows.
/// This is used to receive notifications about new toplevel windows.
#[derive(Default)]
pub struct ForeignToplevelManager {
    pub(crate) on_toplevel: Option<ToplevelCallback>,
}

impl ForeignToplevelManager {
    /// Creates a new foreign toplevel manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a callback to be called when a new toplevel window is created.
    pub fn on_toplevel<F>(&mut self, callback: F)
    where
        F: FnMut(ForeignToplevelHandle) + Send + 'static,
    {
        self.on_toplevel = Some(Box::new(callback));
    }

    pub(crate) fn handle_toplevel(&mut self, handle: ForeignToplevelHandle) {
        if let Some(callback) = &mut self.on_toplevel {
            callback(handle);
        }
    }
}

/// An error indicating that an action failed because the compositor doesn't support the required
/// foreign toplevel management protocol.
#[derive(Debug, Error)]
#[error("Compositor doesn't support zwlr_foreign_toplevel_manager_v1")]
pub struct ForeignToplevelNotSupportedError;
