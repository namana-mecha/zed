#![cfg(all(target_os = "linux", feature = "wayland"))]

use gpui::*;

struct SimpleKeyboard {
    keys: Vec<Vec<&'static str>>,
}

impl SimpleKeyboard {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            keys: vec![
                vec!["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
                vec!["a", "s", "d", "f", "g", "h", "j", "k", "l"],
                vec!["z", "x", "c", "v", "b", "n", "m", "Space"],
            ],
        }
    }

    fn handle_key_press(&mut self, key: &str, window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(im) = window.get_input_method() {
            if im.is_active() {
                let text = if key == "Space" { " " } else { key };
                im.commit_string(text);
                im.commit();
            }
        }
    }
}

impl Render for SimpleKeyboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = window
            .get_input_method()
            .as_ref()
            .map_or(false, |im| im.is_active());

        let status = if is_active { "Active" } else { "Inactive" };

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
                    .child(format!("Input Method: {}", status)),
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
                    keyboard_interactivity: layer_shell::KeyboardInteractivity::OnDemand,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(SimpleKeyboard::new),
        )
        .unwrap();
    });
}
