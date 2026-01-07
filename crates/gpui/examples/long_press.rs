use gpui::{
    App, Application, Bounds, Context, LongPressEvent, Render, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};

struct LongPressDemo {
    last_action: SharedString,
    click_count: usize,
    long_press_count: usize,
}

impl LongPressDemo {
    fn new() -> Self {
        Self {
            last_action: "Click or long press a button below!".into(),
            click_count: 0,
            long_press_count: 0,
        }
    }

    fn on_click(&mut self, button: &str, _window: &mut Window, cx: &mut Context<Self>) {
        self.click_count += 1;
        self.last_action = format!("Clicked '{}' button (total clicks: {})", button, self.click_count).into();
        cx.notify();
    }

    fn on_long_press(&mut self, button: &str, duration_ms: u64, _window: &mut Window, cx: &mut Context<Self>) {
        self.long_press_count += 1;
        self.last_action = format!(
            "Long pressed '{}' button for {}ms (total long presses: {})",
            button, duration_ms, self.long_press_count
        )
        .into();
        cx.notify();
    }
}

impl Render for LongPressDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(0x2d2d2d))
            .size_full()
            .p_8()
            .child(
                // Title
                div()
                    .text_2xl()
                    .text_color(rgb(0xffffff))
                    .child("Long Press Demo"),
            )
            .child(
                // Instructions
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0x3d3d3d))
                    .rounded_lg()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xcccccc))
                            .child("Instructions:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("• Click quickly: fires click event only"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("• Hold for 500ms+: fires long press, then click on release"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("• Move mouse while holding: cancels long press"),
                    ),
            )
            .child(
                // Status display
                div()
                    .p_4()
                    .bg(rgb(0x1a1a1a))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(0x4a4a4a))
                    .text_sm()
                    .text_color(rgb(0x00ff00))
                    .child(self.last_action.clone()),
            )
            .child(
                // Buttons
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xcccccc))
                            .child("Default Duration (500ms):"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(self.create_button("Primary", rgb(0x0066cc).into(), cx))
                            .child(self.create_button("Success", rgb(0x00aa00).into(), cx))
                            .child(self.create_button("Danger", rgb(0xcc0000).into(), cx)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xcccccc))
                            .mt_4()
                            .child("Custom Duration (1000ms):"),
                    )
                    .child(
                        self.create_custom_duration_button("Slow Press", rgb(0xff9900).into(), 1000, cx),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xcccccc))
                            .mt_4()
                            .child("Custom Duration (200ms):"),
                    )
                    .child(
                        self.create_custom_duration_button("Fast Press", rgb(0xff00ff).into(), 200, cx),
                    ),
            )
            .child(
                // Statistics
                div()
                    .flex()
                    .gap_6()
                    .mt_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child("Total Clicks:"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .text_color(rgb(0xffffff))
                                    .child(format!("{}", self.click_count)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child("Total Long Presses:"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .text_color(rgb(0xffffff))
                                    .child(format!("{}", self.long_press_count)),
                            ),
                    ),
            )
    }
}

impl LongPressDemo {
    fn create_button(
        &self,
        label: &str,
        color: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_string();
        let id: SharedString = label.clone().into();
        div()
            .id(id)
            .px_6()
            .py_3()
            .bg(color)
            .rounded_lg()
            .text_color(rgb(0xffffff))
            .cursor_pointer()
            .hover(|style| style.opacity(0.8))
            .active(|style| style.opacity(0.6))
            .on_click({
                let label = label.clone();
                cx.listener(move |this, _event, window, cx| {
                    this.on_click(&label, window, cx);
                })
            })
            .on_long_press({
                let label = label.clone();
                cx.listener(move |this, event: &LongPressEvent, window, cx| {
                    this.on_long_press(&label, event.duration().as_millis() as u64, window, cx);
                })
            })
            .child(label)
    }

    fn create_custom_duration_button(
        &self,
        label: &str,
        color: gpui::Hsla,
        duration_ms: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_string();
        let id: SharedString = format!("{}-{}", label, duration_ms).into();
        div()
            .id(id)
            .px_6()
            .py_3()
            .bg(color)
            .rounded_lg()
            .text_color(rgb(0xffffff))
            .cursor_pointer()
            .hover(|style| style.opacity(0.8))
            .active(|style| style.opacity(0.6))
            .on_click({
                let label = label.clone();
                cx.listener(move |this, _event, window, cx| {
                    this.on_click(&label, window, cx);
                })
            })
            .on_long_press_ms(duration_ms, {
                let label = label.clone();
                cx.listener(move |this, event: &LongPressEvent, window, cx| {
                    this.on_long_press(&label, event.duration().as_millis() as u64, window, cx);
                })
            })
            .child(format!("{} ({}ms)", label, duration_ms))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(600.), px(700.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Long Press Demo".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| LongPressDemo::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}
