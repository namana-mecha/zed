use std::cell::RefCell;
use std::rc::Rc;

use util::ResultExt;
use wayland_backend::protocol::WEnum;

use super::client::WaylandClientState;

/// Key state for virtual keyboard events.
pub enum KeyState {
    /// Key released.
    Released = 0,
    /// Key pressed.
    Pressed = 1,
}

/// Handle to the virtual keyboard protocol.
///
/// Allows sending keyboard events to the compositor.
#[derive(Clone)]
pub struct VirtualKeyboardHandle {
    client: Rc<RefCell<WaylandClientState>>,
}

impl VirtualKeyboardHandle {
    pub(crate) fn new(client: Rc<RefCell<WaylandClientState>>) -> Self {
        Self { client }
    }

    /// Send a key event to the compositor.
    ///
    /// # Arguments
    /// * `time` - Timestamp in milliseconds
    /// * `key` - Key code (Linux evdev keycode minus 8)
    /// * `state` - Whether the key was pressed or released
    pub fn send_key(&self, time: u32, key: u32, state: KeyState) {
        let client = self.client.borrow();
        if let Some(ref vk) = client.virtual_keyboard {
            vk.key(time, key, WEnum::Value(state as u32).into());
            // Flush to ensure the event is sent immediately
            client.connection.flush().log_err();
        }
    }

    /// Send modifier state to the compositor.
    ///
    /// # Arguments
    /// * `mods_depressed` - Depressed modifiers
    /// * `mods_latched` - Latched modifiers
    /// * `mods_locked` - Locked modifiers
    /// * `group` - Keyboard layout group
    pub fn send_modifiers(
        &self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        let client = self.client.borrow();
        if let Some(ref vk) = client.virtual_keyboard {
            vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
            // Flush to ensure the event is sent immediately
            client.connection.flush().log_err();
        }
    }
}

/// Handle to the input method protocol.
///
/// Allows an application to act as an input method (IME) and send
/// text input events to other applications.
#[derive(Clone)]
pub struct InputMethodHandle {
    client: Rc<RefCell<WaylandClientState>>,
}

impl InputMethodHandle {
    pub(crate) fn new(client: Rc<RefCell<WaylandClientState>>) -> Self {
        Self { client }
    }

    /// Check if the input method is currently active.
    ///
    /// Returns `true` if the input method has been activated by the compositor.
    pub fn is_active(&self) -> bool {
        self.client.borrow().input_method_active
    }

    /// Commit a string to the currently focused text input.
    ///
    /// # Arguments
    /// * `text` - The text to commit
    pub fn commit_string(&self, text: &str) {
        let client = self.client.borrow();
        if let Some(ref im) = client.input_method {
            im.commit_string(text.to_string());
            // Flush to ensure the event is sent immediately
            client.connection.flush().log_err();
        }
    }

    /// Set pre-edit (composition) text.
    ///
    /// # Arguments
    /// * `text` - The pre-edit text
    /// * `cursor_begin` - Start of cursor position in the pre-edit text
    /// * `cursor_end` - End of cursor position in the pre-edit text
    pub fn set_preedit(&self, text: &str, cursor_begin: i32, cursor_end: i32) {
        let client = self.client.borrow();
        if let Some(ref im) = client.input_method {
            im.set_preedit_string(text.to_string(), cursor_begin, cursor_end);
            // Flush to ensure the event is sent immediately
            client.connection.flush().log_err();
        }
    }

    /// Delete text surrounding the cursor.
    ///
    /// # Arguments
    /// * `before_length` - Number of bytes to delete before the cursor
    /// * `after_length` - Number of bytes to delete after the cursor
    pub fn delete_surrounding_text(&self, before_length: u32, after_length: u32) {
        let client = self.client.borrow();
        if let Some(ref im) = client.input_method {
            im.delete_surrounding_text(before_length, after_length);
            // Flush to ensure the event is sent immediately
            client.connection.flush().log_err();
        }
    }

    /// Commit the current input method state.
    ///
    /// This should be called after sending text or pre-edit changes.
    pub fn commit(&self) {
        let client = self.client.borrow();
        if let Some(ref im) = client.input_method {
            let serial = client
                .serial_tracker
                .get(super::serial::SerialKind::InputMethod);
            im.commit(serial);
            // Flush to ensure the event is sent immediately
            client.connection.flush().log_err();
        }
    }

    /// Get the surrounding text from the currently focused text input.
    ///
    /// Returns a tuple of (text, cursor position, anchor position) if available.
    pub fn get_surrounding_text(&self) -> Option<(String, u32, u32)> {
        self.client.borrow().surrounding_text.clone()
    }

    /// Get the content type hints for the currently focused text input.
    ///
    /// Returns a tuple of (content hint, content purpose).
    pub fn get_content_type(&self) -> (u32, u32) {
        self.client.borrow().content_type
    }
}
