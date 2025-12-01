fn main() {
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    example::main();

    #[cfg(not(all(target_os = "linux", feature = "wayland")))]
    panic!("This example requires the `wayland` feature and a linux system.");
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod example {
    use gpui::{
        actions, div, prelude::*, px, rems, rgba, white, App, Application, Bounds, Context,
        FontWeight, MouseButton, Point, Size, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds,
        WindowHandle, WindowKind, WindowOptions, session_lock::*,
    };

    actions!(session_lock_example, [Lock]);

    struct SessionLockExample {
        lock_window: Option<WindowHandle<SessionLockWindow>>,
    }

    impl SessionLockExample {
        fn new(_cx: &mut Context<Self>) -> Self {
            SessionLockExample { lock_window: None }
        }

        fn lock(&mut self, _: &Lock, _window: &mut Window, cx: &mut Context<Self>) {
            log::info!("Locking session...");

            let handle = cx.entity().downgrade();

            let lock_window = cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::new(px(0.), px(0.)),
                        size: Size::new(px(800.), px(600.)),
                    })),
                    app_id: Some("gpui-session-lock-example".to_string()),
                    window_background: WindowBackgroundAppearance::Transparent,
                    kind: WindowKind::SessionLock(SessionLockOptions {
                        namespace: Some("gpui-session-lock".to_string()),
                    }),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| SessionLockWindow::new(handle.clone(), cx)),
            );

            if let Ok(window) = lock_window {
                self.lock_window = Some(window);
            }
        }
    }

    impl Render for SessionLockExample {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .bg(rgba(0xF3F4F6FF))
                .child(
                    div()
                        .text_size(rems(2.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgba(0x1F2937FF))
                        .child("Session Lock Example"),
                )
                .child(
                    div()
                        .px_8()
                        .py_4()
                        .rounded_lg()
                        .bg(rgba(0xEF4444FF))
                        .text_color(white())
                        .text_size(rems(1.2))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0xDC2626FF)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.lock(&Lock, window, cx);
                            }),
                        )
                        .child("Lock Screen"),
                )
        }
    }

    struct SessionLockWindow {
        main_window: WeakEntity<SessionLockExample>,
    }

    impl SessionLockWindow {
        fn new(main_window: WeakEntity<SessionLockExample>, _cx: &mut Context<Self>) -> Self {
            SessionLockWindow { main_window }
        }

        fn unlock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
            log::info!("Unlocking session...");

            // Clear the lock window reference in the main window
            if let Ok(main) = self.main_window.update(cx, |main, _cx| {
                main.lock_window = None;
            }) {
                let _ = main;
            }

            window.remove_window();
        }
    }

    impl Render for SessionLockWindow {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .bg(rgba(0x000000CC))
                .child(
                    div()
                        .text_size(rems(3.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(white())
                        .child("Session Locked"),
                )
                .child(
                    div()
                        .px_8()
                        .py_4()
                        .rounded_lg()
                        .bg(rgba(0x3B82F6FF))
                        .text_color(white())
                        .text_size(rems(1.2))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0x2563EBFF)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.unlock(window, cx);
                            }),
                        )
                        .child("Click to Unlock"),
                )
        }
    }

    pub fn main() {
        env_logger::init();

        Application::new().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::new(px(100.), px(100.)),
                        size: Size::new(px(400.), px(300.)),
                    })),
                    app_id: Some("gpui-session-lock-example".to_string()),
                    ..Default::default()
                },
                |_, cx| cx.new(SessionLockExample::new),
            )
            .unwrap();
        });
    }
}
