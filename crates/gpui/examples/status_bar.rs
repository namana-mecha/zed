fn main() {
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    example::main();

    #[cfg(not(all(target_os = "linux", feature = "wayland")))]
    panic!("This example requires the `wayland` feature and a linux system.");
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod example {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use gpui::{
        App, Application, Bounds, Context, FontWeight, InteractiveElement, MouseButton,
        ParentElement, Size, Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
        WindowOptions, div, layer_shell::*, point, prelude::*, px, rems, rgb, size, white,
    };

    struct StatusBar {
        settings_open: bool,
    }

    impl StatusBar {
        fn new(cx: &mut Context<Self>) -> Self {
            // Update clock every second
            cx.spawn(async move |this, cx| {
                loop {
                    let _ = this.update(cx, |_, cx| cx.notify());
                    cx.background_executor()
                        .timer(Duration::from_millis(1000))
                        .await;
                }
            })
            .detach();

            StatusBar {
                settings_open: false,
            }
        }

        fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            self.settings_open = !self.settings_open;
            self.update_input_regions(window, cx);
            cx.notify();
        }

        fn update_input_regions(&self, window: &mut Window, _cx: &mut Context<Self>) {
            let bounds = window.bounds();
            let bar_height = px(40.);
            let clock_width = px(120.);
            let settings_button_width = px(80.);
            let panel_width = px(300.);
            let panel_height = px(400.);

            let mut regions = Vec::new();

            regions.push(Bounds {
                origin: point(px(10.), px(5.)),
                size: size(clock_width, px(30.)),
            });

            let settings_x = bounds.size.width - settings_button_width - px(10.);
            regions.push(Bounds {
                origin: point(settings_x, px(5.)),
                size: size(settings_button_width, px(30.)),
            });

            if self.settings_open {
                let panel_x = bounds.size.width - panel_width - px(10.);
                regions.push(Bounds {
                    origin: point(panel_x, bar_height + px(5.)),
                    size: size(panel_width, panel_height),
                });
            }

            window.set_input_regions(Some(regions));
        }

        fn get_time() -> (u64, u64, u64) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let hours = (now / 3600) % 24;
            let minutes = (now / 60) % 60;
            let seconds = now % 60;

            (hours, minutes, seconds)
        }
    }

    impl Render for StatusBar {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let (hours, minutes, seconds) = Self::get_time();

            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    // Status bar
                    div()
                        .h(px(40.))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_2()
                        .bg(rgb(0x2d2d2d))
                        .shadow_lg()
                        .child(
                            // Clock on the left
                            div()
                                .flex()
                                .items_center()
                                .px_3()
                                .h(px(30.))
                                .rounded_md()
                                .bg(rgb(0x3d3d3d))
                                .text_size(rems(0.9))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(white())
                                .child(format!("{:02}:{:02}:{:02}", hours, minutes, seconds)),
                        )
                        .child(
                            // Settings button on the right
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .px_3()
                                .h(px(30.))
                                .rounded_md()
                                .bg(if self.settings_open {
                                    rgb(0x4d4d4d)
                                } else {
                                    rgb(0x3d3d3d)
                                })
                                .hover(|style| style.bg(rgb(0x4d4d4d)))
                                .text_size(rems(0.9))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(white())
                                .child("Settings")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.toggle_settings(window, cx);
                                    }),
                                ),
                        ),
                )
                .when(self.settings_open, |this| {
                    // Settings panel (slides down from the right)
                    this.child(
                        div()
                            .absolute()
                            .top(px(45.))
                            .right(px(10.))
                            .w(px(300.))
                            .h(px(400.))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_4()
                            .bg(rgb(0x2d2d2d))
                            .rounded_lg()
                            .shadow_xl()
                            .child(
                                div()
                                    .text_size(rems(1.2))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(white())
                                    .child("Settings Panel"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(self.render_setting("Volume", "80%"))
                                    .child(self.render_setting("Brightness", "60%"))
                                    .child(self.render_setting("Wi-Fi", "Connected"))
                                    .child(self.render_setting("Bluetooth", "Off"))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .mt_4()
                                            .px_4()
                                            .h(px(36.))
                                            .rounded_md()
                                            .bg(rgb(0x4d4d4d))
                                            .hover(|style| style.bg(rgb(0x5d5d5d)))
                                            .text_size(rems(0.9))
                                            .text_color(white())
                                            .child("System Settings"),
                                    ),
                            ),
                    )
                })
        }
    }

    impl StatusBar {
        fn render_setting(&self, label: &str, value: &str) -> impl IntoElement {
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .h(px(40.))
                .rounded_md()
                .bg(rgb(0x3d3d3d))
                .child(
                    div()
                        .text_size(rems(0.9))
                        .text_color(rgb(0xcccccc))
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(rems(0.9))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(white())
                        .child(value.to_string()),
                )
        }
    }

    pub fn main() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: Size::new(px(1920.), px(500.)),
                    })),
                    app_id: Some("gpui-status-bar-example".to_string()),
                    window_background: WindowBackgroundAppearance::Transparent,
                    kind: WindowKind::LayerShell(LayerShellOptions {
                        namespace: "gpui-statusbar".to_string(),
                        layer: Layer::Top,
                        anchor: Anchor::LEFT | Anchor::RIGHT | Anchor::TOP,
                        margin: Some((px(0.), px(0.), px(0.), px(0.))),
                        keyboard_interactivity: KeyboardInteractivity::OnDemand,
                        exclusive_zone: Some(px(20.)),
                        exclusive_edge: Some(Anchor::TOP),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let status_bar = cx.new(StatusBar::new);

                    status_bar.update(cx, |bar, cx| {
                        bar.update_input_regions(window, cx);
                    });

                    status_bar
                },
            )
            .unwrap();
        });
    }
}
