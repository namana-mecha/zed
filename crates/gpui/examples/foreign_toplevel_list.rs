fn main() {
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    example::main();

    #[cfg(not(all(target_os = "linux", feature = "wayland")))]
    panic!("This example requires the `wayland` feature and a linux system.");
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod example {
    use gpui::{
        App, Application, Bounds, Context, FontWeight, Render, SharedString, Size, Window,
        WindowBounds, WindowOptions, div, foreign_toplevel_management::ForeignToplevelHandle,
        point, prelude::*, px, rems, rgb, white,
    };

    #[derive(Clone, Debug)]
    struct ToplevelEntry {
        handle: ForeignToplevelHandle,
        title: SharedString,
        app_id: SharedString,
    }

    struct ToplevelList {
        toplevels: Vec<ToplevelEntry>,
    }

    impl ToplevelList {
        fn new(_cx: &mut Context<Self>) -> Self {
            Self {
                toplevels: Vec::new(),
            }
        }

        fn refresh(&mut self, window: &Window) {
            self.toplevels.clear();

            for handle in window.foreign_toplevels() {
                let title = handle.title().unwrap_or("(no title)".into());
                let app_id = handle.app_id().unwrap_or("(unknown)".into());

                self.toplevels.push(ToplevelEntry {
                    handle,
                    title,
                    app_id,
                });
            }
        }

        fn maximize_toplevel(&mut self, index: usize, _cx: &mut Context<Self>) {
            if let Some(entry) = self.toplevels.get(index) {
                if entry.handle.is_maximized() {
                    entry.handle.unset_maximized();
                } else {
                    entry.handle.set_maximized();
                }
            }
        }

        fn minimize_toplevel(&mut self, index: usize, _cx: &mut Context<Self>) {
            if let Some(entry) = self.toplevels.get(index) {
                if entry.handle.is_minimized() {
                    entry.handle.unset_minimized();
                } else {
                    entry.handle.set_minimized();
                }
            }
        }

        fn close_toplevel(&mut self, index: usize, _cx: &mut Context<Self>) {
            if let Some(entry) = self.toplevels.get(index) {
                entry.handle.close();
            }
        }

        fn minimize_all_except_self(&mut self, window: &Window) {
            let our_app_id = "gpui-foreign-toplevel-example";

            for entry in &self.toplevels {
                if let Some(app_id) = entry.handle.app_id() {
                    if app_id.as_ref() != our_app_id && !entry.handle.is_minimized() {
                        entry.handle.set_minimized();
                    }
                }
            }
        }
    }

    impl Render for ToplevelList {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .p_4()
                .bg(rgb(0x1e1e2e))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb_4()
                        .child(
                            div()
                                .text_size(rems(1.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(white())
                                .child(format!(
                                    "Foreign Toplevel Windows ({})",
                                    self.toplevels.len()
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0x89b4fa))
                                        .hover(|style| style.bg(rgb(0x7aa2f7)))
                                        .text_color(rgb(0x1e1e2e))
                                        .text_size(rems(0.9))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(|this, _event, window, cx| {
                                                this.refresh(window);
                                                cx.notify();
                                            }),
                                        )
                                        .child("Refresh"),
                                )
                                .child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0xf9e2af))
                                        .hover(|style| style.bg(rgb(0xf5d08a)))
                                        .text_color(rgb(0x1e1e2e))
                                        .text_size(rems(0.9))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(|this, _event, window, cx| {
                                                this.minimize_all_except_self(window);
                                            }),
                                        )
                                        .child("Minimize All Others"),
                                ),
                        ),
                )
                .child(div().flex().flex_col().gap_2().children(
                    self.toplevels.iter().enumerate().map(|(i, entry)| {
                        let is_maximized = entry.handle.is_maximized();
                        let is_minimized = entry.handle.is_minimized();
                        let is_activated = entry.handle.is_activated();

                        div()
                            .flex()
                            .flex_col()
                            .p_3()
                            .rounded_md()
                            .bg(if is_activated {
                                rgb(0x313244)
                            } else {
                                rgb(0x252535)
                            })
                            .border_1()
                            .border_color(rgb(0x45475a))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(rems(1.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(white())
                                                    .child(entry.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(rems(0.8))
                                                    .text_color(rgb(0x9399b2))
                                                    .child(format!("App: {}", entry.app_id)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(rems(0.7))
                                                    .text_color(rgb(0x6c7086))
                                                    .child(format!(
                                                        "State: {}{}{}",
                                                        if is_maximized {
                                                            "Maximized "
                                                        } else {
                                                            ""
                                                        },
                                                        if is_minimized {
                                                            "Minimized "
                                                        } else {
                                                            ""
                                                        },
                                                        if is_activated {
                                                            "Active"
                                                        } else {
                                                            "Inactive"
                                                        }
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x45475a))
                                                    .hover(|style| style.bg(rgb(0x585b70)))
                                                    .text_color(white())
                                                    .text_size(rems(0.8))
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(
                                                            move |this, _event, _window, cx| {
                                                                this.maximize_toplevel(i, cx);
                                                            },
                                                        ),
                                                    )
                                                    .child(if is_maximized {
                                                        "Restore"
                                                    } else {
                                                        "Maximize"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0x45475a))
                                                    .hover(|style| style.bg(rgb(0x585b70)))
                                                    .text_color(white())
                                                    .text_size(rems(0.8))
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(
                                                            move |this, _event, _window, cx| {
                                                                this.minimize_toplevel(i, cx);
                                                            },
                                                        ),
                                                    )
                                                    .child(if is_minimized {
                                                        "Unminimize"
                                                    } else {
                                                        "Minimize"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(rgb(0xf38ba8))
                                                    .hover(|style| style.bg(rgb(0xf07996)))
                                                    .text_color(white())
                                                    .text_size(rems(0.8))
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(
                                                            move |this, _event, _window, cx| {
                                                                this.close_toplevel(i, cx);
                                                            },
                                                        ),
                                                    )
                                                    .child("Close"),
                                            ),
                                    ),
                            )
                    }),
                ))
        }
    }

    pub fn main() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(100.), px(100.)),
                        size: Size::new(px(800.), px(600.)),
                    })),
                    app_id: Some("gpui-foreign-toplevel-example".to_string()),
                    ..Default::default()
                },
                |window, cx| {
                    let mut view = cx.new(ToplevelList::new);
                    // Initial refresh to populate the list
                    view.update(cx, |this, cx| {
                        this.refresh(window);
                        cx.notify();
                    });
                    view
                },
            )
            .unwrap();
        });
    }
}
