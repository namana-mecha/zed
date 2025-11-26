use gpui::{
    Application, Background, BorderStyle, Context, Hsla, MouseDownEvent, Pixels, Point, Render,
    Window, WindowOptions, canvas, div, point, prelude::*, px, rgb, polygon,
};

#[derive(Clone)]
struct PolygonStyle {
    points: Vec<Point<Pixels>>,
    background: Background,
    border_width: Pixels,
    border_color: Hsla,
    border_style: BorderStyle,
}

struct PolygonViewer {
    polygons: Vec<PolygonStyle>,
}

impl PolygonViewer {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let mut polygons = vec![];

        // Row 1: Triangles (filled, with border, transparent)
        let base_x = 80.;
        let base_y = 80.;
        let spacing = 200.;
        
        // Triangle 1: Filled red
        polygons.push(PolygonStyle {
            points: vec![
                point(px(base_x), px(base_y - 40.)),
                point(px(base_x + 50.), px(base_y + 40.)),
                point(px(base_x - 50.), px(base_y + 40.)),
            ],
            background: rgb(0xFF0000).into(),
            border_width: px(0.),
            border_color: rgb(0x000000).into(),
            border_style: BorderStyle::Solid,
        });

        // Triangle 2: Blue with black border
        polygons.push(PolygonStyle {
            points: vec![
                point(px(base_x + spacing), px(base_y - 40.)),
                point(px(base_x + spacing + 50.), px(base_y + 40.)),
                point(px(base_x + spacing - 50.), px(base_y + 40.)),
            ],
            background: rgb(0x4169E1).into(),
            border_width: px(3.),
            border_color: rgb(0x000000).into(),
            border_style: BorderStyle::Solid,
        });

        // Triangle 3: Transparent with dashed border
        polygons.push(PolygonStyle {
            points: vec![
                point(px(base_x + spacing * 2.), px(base_y - 40.)),
                point(px(base_x + spacing * 2. + 50.), px(base_y + 40.)),
                point(px(base_x + spacing * 2. - 50.), px(base_y + 40.)),
            ],
            background: {
                let mut color = rgb(0xFFFFFF);
                color.a = 0.3;
                color.into()
            },
            border_width: px(3.),
            border_color: rgb(0xFF0000).into(),
            border_style: BorderStyle::Dashed,
        });

        // Row 2: Pentagons
        let base_y = 250.;
        
        // Pentagon 1: Filled green
        let mut pentagon_points = vec![];
        for i in 0..5 {
            let angle = (i as f32 * 2.0 * std::f32::consts::PI / 5.0) - std::f32::consts::PI / 2.0;
            pentagon_points.push(point(
                px(base_x + 50. * angle.cos()),
                px(base_y + 50. * angle.sin()),
            ));
        }
        polygons.push(PolygonStyle {
            points: pentagon_points,
            background: rgb(0x00AA00).into(),
            border_width: px(0.),
            border_color: rgb(0x000000).into(),
            border_style: BorderStyle::Solid,
        });

        // Pentagon 2: Yellow with border
        let mut pentagon_points = vec![];
        for i in 0..5 {
            let angle = (i as f32 * 2.0 * std::f32::consts::PI / 5.0) - std::f32::consts::PI / 2.0;
            pentagon_points.push(point(
                px(base_x + spacing + 50. * angle.cos()),
                px(base_y + 50. * angle.sin()),
            ));
        }
        polygons.push(PolygonStyle {
            points: pentagon_points,
            background: rgb(0xFFD700).into(),
            border_width: px(4.),
            border_color: rgb(0x8B4513).into(),
            border_style: BorderStyle::Solid,
        });

        // Pentagon 3: Gradient with dashed border
        let mut pentagon_points = vec![];
        for i in 0..5 {
            let angle = (i as f32 * 2.0 * std::f32::consts::PI / 5.0) - std::f32::consts::PI / 2.0;
            pentagon_points.push(point(
                px(base_x + spacing * 2. + 50. * angle.cos()),
                px(base_y + 50. * angle.sin()),
            ));
        }
        polygons.push(PolygonStyle {
            points: pentagon_points,
            background: rgb(0xFF69B4).into(),
            border_width: px(3.),
            border_color: rgb(0x8B008B).into(),
            border_style: BorderStyle::Dashed,
        });

        // Row 3: Stars
        let base_y = 420.;
        
        // Star 1: Filled purple
        let mut star_points = vec![];
        for i in 0..10 {
            let angle = (i as f32 * std::f32::consts::PI / 5.0) - std::f32::consts::PI / 2.0;
            let radius = if i % 2 == 0 { 50. } else { 20. };
            star_points.push(point(
                px(base_x + radius * angle.cos()),
                px(base_y + radius * angle.sin()),
            ));
        }
        polygons.push(PolygonStyle {
            points: star_points,
            background: rgb(0x9370DB).into(),
            border_width: px(0.),
            border_color: rgb(0x000000).into(),
            border_style: BorderStyle::Solid,
        });

        // Star 2: Orange with border
        let mut star_points = vec![];
        for i in 0..10 {
            let angle = (i as f32 * std::f32::consts::PI / 5.0) - std::f32::consts::PI / 2.0;
            let radius = if i % 2 == 0 { 50. } else { 20. };
            star_points.push(point(
                px(base_x + spacing + radius * angle.cos()),
                px(base_y + radius * angle.sin()),
            ));
        }
        polygons.push(PolygonStyle {
            points: star_points,
            background: rgb(0xFF8C00).into(),
            border_width: px(2.),
            border_color: rgb(0x000000).into(),
            border_style: BorderStyle::Solid,
        });

        // Star 3: Cyan with thick dashed border
        let mut star_points = vec![];
        for i in 0..10 {
            let angle = (i as f32 * std::f32::consts::PI / 5.0) - std::f32::consts::PI / 2.0;
            let radius = if i % 2 == 0 { 50. } else { 20. };
            star_points.push(point(
                px(base_x + spacing * 2. + radius * angle.cos()),
                px(base_y + radius * angle.sin()),
            ));
        }
        polygons.push(PolygonStyle {
            points: star_points,
            background: rgb(0x00CED1).into(),
            border_width: px(3.),
            border_color: rgb(0x006400).into(),
            border_style: BorderStyle::Dashed,
        });

        Self { polygons }
    }
}

impl Render for PolygonViewer {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let polygons = self.polygons.clone();

        div()
            .bg(gpui::white())
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .justify_between()
                    .items_center()
                    .child("Polygon Rendering Examples - 3 Rows: Triangles, Pentagons, Stars"),
            )
            .child(
                div()
                    .size_full()
                    .child(
                        canvas(
                            move |_, _, _| {},
                            move |_, _, window, _| {
                                for style in &polygons {
                                    window.paint_polygon(
                                        polygon(style.points.clone(), style.background)
                                            .border_width(style.border_width)
                                            .border_color(style.border_color)
                                            .border_style(style.border_style)
                                    );
                                }
                            },
                        )
                        .size_full(),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        |_: &MouseDownEvent, _, _| {
                            // Handle mouse down if needed
                        },
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx| {
        cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| PolygonViewer::new(window, cx)),
        )
        .unwrap();
        cx.on_window_closed(|cx| {
            cx.quit();
        })
        .detach();
        cx.activate(true);
    });
}






