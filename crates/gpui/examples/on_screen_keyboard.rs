#![cfg(all(target_os = "linux", feature = "wayland"))]

use gpui::*;
use gpui::input_method::KeyState;

struct SimpleKeyboard {
    keys: Vec<Vec<&'static str>>,
    was_active: bool,
    _poll_task: gpui::Task<()>,
}

// Map character to Linux evdev keycode (already minus 8 for Wayland)
fn char_to_keycode(c: char) -> Option<u32> {
    match c.to_ascii_lowercase() {
        'q' => Some(16 - 8),
        'w' => Some(17 - 8),
        'e' => Some(18 - 8),
        'r' => Some(19 - 8),
        't' => Some(20 - 8),
        'y' => Some(21 - 8),
        'u' => Some(22 - 8),
        'i' => Some(23 - 8),
        'o' => Some(24 - 8),
        'p' => Some(25 - 8),
        'a' => Some(30 - 8),
        's' => Some(31 - 8),
        'd' => Some(32 - 8),
        'f' => Some(33 - 8),
        'g' => Some(34 - 8),
        'h' => Some(35 - 8),
        'j' => Some(36 - 8),
        'k' => Some(37 - 8),
        'l' => Some(38 - 8),
        'z' => Some(44 - 8),
        'x' => Some(45 - 8),
        'c' => Some(46 - 8),
        'v' => Some(47 - 8),
        'b' => Some(48 - 8),
        'n' => Some(49 - 8),
        'm' => Some(50 - 8),
        ' ' => Some(57 - 8), // Space bar
        _ => None,
    }
}

impl SimpleKeyboard {
    fn new(cx: &mut Context<Self>) -> Self {
        let poll_task = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_spawn(async {
                    smol::Timer::after(std::time::Duration::from_millis(100)).await;
                })
                .await;

                let _ = this.update(cx, |_this, cx| {
                    cx.notify();
                });
            }
        });

        Self {
            keys: vec![
                vec!["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
                vec!["a", "s", "d", "f", "g", "h", "j", "k", "l"],
                vec!["z", "x", "c", "v", "b", "n", "m", "Space"],
            ],
            was_active: false,
            _poll_task: poll_task,
        }
    }

    fn handle_key_press(&mut self, key: &str, window: &mut Window, _cx: &mut Context<Self>) {
        eprintln!("Key pressed: {}", key);
        if let Some(vk) = window.get_virtual_keyboard() {
            eprintln!("Virtual keyboard exists");
            let text = if key == "Space" { " " } else { key };

            if let Some(keycode) = char_to_keycode(text.chars().next().unwrap()) {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u32;

                eprintln!("Sending keycode {} (press + release)", keycode);

                // Send key press
                vk.send_key(timestamp, keycode, KeyState::Pressed);

                // Send key release
                vk.send_key(timestamp + 50, keycode, KeyState::Released);
            } else {
                eprintln!("No keycode mapping for character: {}", text);
            }
        } else {
            eprintln!("No virtual keyboard available");
        }
    }
}

impl Render for SimpleKeyboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Show keyboard when input method is active (text field has focus)
        let is_active = window.get_virtual_keyboard().is_some() && window.is_input_method_active();

        if is_active != self.was_active {
            self.was_active = is_active;
            eprintln!("Input method activation changed to: {}", is_active);

            if is_active {
                eprintln!("Showing keyboard (text input focused)");
                window.resize(size(px(600.), px(250.)));
            } else {
                eprintln!("Hiding keyboard (no text input focused)");
                window.resize(size(px(1.), px(1.)));
            }

            cx.notify();
        }

        let status = if is_active {
            "Active - Text Input Focused"
        } else {
            "Inactive - No Text Input"
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .p_4()
            .bg(rgb(0x2e3440))
            .child(
                div()
                    .text_color(rgb(0xeceff4))
                    .text_size(px(14.))
                    .child(format!("Virtual Keyboard: {}", status)),
            )
            .children(self.keys.iter().map(|row| {
                div()
                    .flex()
                    .gap_2()
                    .children(row.iter().map(|key| {
                        let key_str = *key;
                        let width = if key_str == "Space" {
                            px(200.)
                        } else {
                            px(50.)
                        };

                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(width)
                            .h(px(50.))
                            .bg(rgb(0x4c566a))
                            .rounded(px(4.))
                            .text_color(rgb(0xeceff4))
                            .text_size(px(18.))
                            .hover(|style| style.bg(rgb(0x5e81ac)))
                            .child(key_str)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _event, window, cx| {
                                    this.handle_key_press(key_str, window, cx);
                                }),
                            )
                    }))
            }))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(600.), px(250.)),
                })),
                app_id: Some("gpui-simple-keyboard".into()),
                window_background: WindowBackgroundAppearance::Transparent,
                is_movable: false,
                is_resizable: false,
                kind: WindowKind::LayerShell(layer_shell::LayerShellOptions {
                    namespace: "simple-keyboard".into(),
                    layer: layer_shell::Layer::Overlay,
                    anchor: layer_shell::Anchor::BOTTOM
                        | layer_shell::Anchor::LEFT
                        | layer_shell::Anchor::RIGHT,
                    keyboard_interactivity: layer_shell::KeyboardInteractivity::None,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(SimpleKeyboard::new),
        )
        .unwrap();
    });
}
