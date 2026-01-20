use gpui::{prelude::*, *};
use rand::prelude::*;
use std::time::{Duration, Instant};

struct QuadBenchmark {
    current_scene: usize,
    last_frame_time: Instant,
    frame_times: Vec<Duration>,
    fps: f64,
    scene_start_time: Instant,
    rectangles: Vec<RectangleData>,
    rng: StdRng,
}

#[derive(Clone)]
struct RectangleData {
    index: usize,
    x: f32,
    y: f32,
    base_width: f32,
    base_height: f32,
    base_hue: f32,
    border_base_hue: f32,
    base_corner_radius: f32,
    base_border_width: f32,
    changing: bool,
}

impl RectangleData {
    fn new(index: usize, x: f32, y: f32, width: f32, height: f32, rng: &mut StdRng) -> Self {
        Self {
            index,
            x,
            y,
            base_width: width,
            base_height: height,
            base_hue: rng.random::<f32>(),
            border_base_hue: rng.random::<f32>(),
            base_corner_radius: 0.0,
            base_border_width: 0.0,
            changing: false,
        }
    }
}

const SCENE_COUNT: usize = 18;
const SCENE_DURATION: f64 = 5.0;

// Window dimensions - change these to scale the entire benchmark
const WINDOW_WIDTH: f32 = 500.0;
const WINDOW_HEIGHT: f32 = 500.0;

// Derived constants for positioning
const WINDOW_CENTER_X: f32 = WINDOW_WIDTH / 2.0;
const WINDOW_CENTER_Y: f32 = WINDOW_HEIGHT / 2.0;

impl QuadBenchmark {
    fn new() -> Self {
        let rng = StdRng::seed_from_u64(42);
        Self {
            current_scene: 0,
            last_frame_time: Instant::now(),
            frame_times: Vec::new(),
            fps: 0.0,
            scene_start_time: Instant::now(),
            rectangles: Vec::new(),
            rng,
        }
    }

    fn setup_scene(&mut self) {
        self.rectangles.clear();
        self.scene_start_time = Instant::now();

        match self.current_scene {
            // Scene 0: Single rectangle changing color
            0 => {
                let size = WINDOW_WIDTH * 0.2;
                self.rectangles.push(RectangleData::new(
                    0,
                    WINDOW_CENTER_X - size / 2.0,
                    WINDOW_CENTER_Y - size / 2.0,
                    size,
                    size,
                    &mut self.rng,
                ));
                self.rectangles[0].changing = true;
            }

            // Scene 1: Grid with few changing in small area
            1 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        self.rectangles.push(RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        ));
                    }
                }
                for i in 0..16 {
                    self.rectangles[i].changing = true;
                }
            }

            // Scene 2: Grid with few changing scattered
            2 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        self.rectangles.push(RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        ));
                    }
                }
                for i in 0..400 {
                    if i % 25 == 0 {
                        self.rectangles[i].changing = true;
                    }
                }
            }

            // Scene 3: Large grid with size changes
            3 => {
                let grid_size = 25;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.875;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        self.rectangles.push(RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        ));
                    }
                }
                for i in 0..25 {
                    self.rectangles[i].changing = true;
                }
            }

            // Scene 4: Grid with borders, few changing in small area
            4 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 2.0;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..16 {
                    self.rectangles[i].changing = true;
                }
            }

            // Scene 5: Grid with borders, few changing scattered
            5 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 2.0;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..400 {
                    if i % 25 == 0 {
                        self.rectangles[i].changing = true;
                    }
                }
            }

            // Scene 6: Changing border widths
            6 => {
                let grid_size = 15;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 1.0;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..225 {
                    if i % 3 == 0 {
                        self.rectangles[i].changing = true;
                    }
                }
            }

            // Scene 7: Rounded rectangles, few changing in small area
            7 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_corner_radius = rect_size * 0.18;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..16 {
                    self.rectangles[i].changing = true;
                }
            }

            // Scene 8: Rounded rectangles, few changing scattered
            8 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_corner_radius = rect_size * 0.18;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..400 {
                    if i % 25 == 0 {
                        self.rectangles[i].changing = true;
                    }
                }
            }

            // Scene 9: Changing corner radius
            9 => {
                let grid_size = 15;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_corner_radius = rect_size * 0.1;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..225 {
                    if i % 5 == 0 {
                        self.rectangles[i].changing = true;
                    }
                }
            }

            // Scene 10: Borders & radius, few changing in small area
            10 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 2.0;
                        rect.base_corner_radius = rect_size * 0.18;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..16 {
                    self.rectangles[i].changing = true;
                }
            }

            // Scene 11: Borders & radius, few changing scattered
            11 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 2.0;
                        rect.base_corner_radius = rect_size * 0.18;
                        self.rectangles.push(rect);
                    }
                }
                for i in 0..400 {
                    if i % 25 == 0 {
                        self.rectangles[i].changing = true;
                    }
                }
            }

            // Scene 12: Everything changing
            12 => {
                let grid_size = 12;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 2.0;
                        rect.base_corner_radius = rect_size * 0.15;
                        rect.changing = true;
                        self.rectangles.push(rect);
                    }
                }
            }

            // Scene 13: Large rectangles with all properties changing
            13 => {
                let cols = 6;
                let rows = 4;
                let rect_width = (WINDOW_WIDTH / cols as f32) * 0.9;
                let rect_height = (WINDOW_HEIGHT / rows as f32) * 0.9;
                let spacing_x = WINDOW_WIDTH / cols as f32;
                let spacing_y = WINDOW_HEIGHT / rows as f32;
                let margin = spacing_x * 0.05;

                for y in 0..rows {
                    for x in 0..cols {
                        let mut rect = RectangleData::new(
                            y * cols + x,
                            x as f32 * spacing_x + margin,
                            y as f32 * spacing_y + margin,
                            rect_width,
                            rect_height,
                            &mut self.rng,
                        );
                        rect.base_border_width = 3.0;
                        rect.base_corner_radius = rect_width.min(rect_height) * 0.1;
                        rect.changing = true;
                        self.rectangles.push(rect);
                    }
                }
            }

            // Scene 14: Alpha/opacity changes - many rectangles
            14 => {
                let grid_size = 20;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.changing = true;
                        self.rectangles.push(rect);
                    }
                }
            }

            // Scene 15: Alpha changes with borders and radius
            15 => {
                let grid_size = 15;
                let spacing = WINDOW_WIDTH / grid_size as f32;
                let rect_size = spacing * 0.9;

                for y in 0..grid_size {
                    for x in 0..grid_size {
                        let mut rect = RectangleData::new(
                            y * grid_size + x,
                            x as f32 * spacing,
                            y as f32 * spacing,
                            rect_size,
                            rect_size,
                            &mut self.rng,
                        );
                        rect.base_border_width = 2.0;
                        rect.base_corner_radius = rect_size * 0.18;
                        rect.changing = true;
                        self.rectangles.push(rect);
                    }
                }
            }

            // Scene 16: Moving rectangles - horizontal movement
            16 => {
                let count = 30;
                let rect_width = WINDOW_WIDTH * 0.16;
                let rect_height = WINDOW_HEIGHT / count as f32 * 0.85;
                let start_x = WINDOW_WIDTH * 0.1;

                for i in 0..count {
                    let mut rect = RectangleData::new(
                        i,
                        start_x,
                        i as f32 * (WINDOW_HEIGHT / count as f32),
                        rect_width,
                        rect_height,
                        &mut self.rng,
                    );
                    rect.base_corner_radius = rect_height * 0.15;
                    rect.changing = true;
                    self.rectangles.push(rect);
                }
            }

            // Scene 17: Moving rectangles - circular motion
            17 => {
                let count = 25;
                let radius = WINDOW_WIDTH * 0.3;
                let rect_size = WINDOW_WIDTH * 0.12;

                for i in 0..count {
                    let angle = (i as f32) * std::f32::consts::PI * 2.0 / count as f32;
                    let mut rect = RectangleData::new(
                        i,
                        WINDOW_CENTER_X + radius * angle.cos(),
                        WINDOW_CENTER_Y + radius * angle.sin(),
                        rect_size,
                        rect_size,
                        &mut self.rng,
                    );
                    rect.base_corner_radius = rect_size * 0.13;
                    rect.base_border_width = 2.0;
                    rect.changing = true;
                    self.rectangles.push(rect);
                }
            }

            _ => {}
        }
    }

    fn get_scene_description(&self) -> String {
        let changing_count = self.rectangles.iter().filter(|r| r.changing).count();
        match self.current_scene {
            0 => "Scene 0: Single rectangle changing color".to_string(),
            1 => format!(
                "Scene 1: {} rectangles, {} changing in small area",
                self.rectangles.len(),
                changing_count
            ),
            2 => format!(
                "Scene 2: {} rectangles, {} changing scattered",
                self.rectangles.len(),
                changing_count
            ),
            3 => format!(
                "Scene 3: {} rectangles, {} changing sizes in small area",
                self.rectangles.len(),
                changing_count
            ),
            4 => format!(
                "Scene 4: {} rectangles with borders, {} changing in small area",
                self.rectangles.len(),
                changing_count
            ),
            5 => format!(
                "Scene 5: {} rectangles with borders, {} changing scattered",
                self.rectangles.len(),
                changing_count
            ),
            6 => format!(
                "Scene 6: {} rectangles, {} changing border widths",
                self.rectangles.len(),
                changing_count
            ),
            7 => format!(
                "Scene 7: {} rounded rectangles, {} changing in small area",
                self.rectangles.len(),
                changing_count
            ),
            8 => format!(
                "Scene 8: {} rounded rectangles, {} changing scattered",
                self.rectangles.len(),
                changing_count
            ),
            9 => format!(
                "Scene 9: {} rectangles, {} changing corner radius",
                self.rectangles.len(),
                changing_count
            ),
            10 => format!(
                "Scene 10: {} rectangles with borders & radius, {} changing in small area",
                self.rectangles.len(),
                changing_count
            ),
            11 => format!(
                "Scene 11: {} rectangles with borders & radius, {} changing scattered",
                self.rectangles.len(),
                changing_count
            ),
            12 => format!(
                "Scene 12: {} rectangles, everything changing",
                self.rectangles.len()
            ),
            13 => format!(
                "Scene 13: {} large rectangles, all properties changing",
                self.rectangles.len()
            ),
            14 => format!(
                "Scene 14: {} rectangles with alpha/opacity changes",
                self.rectangles.len()
            ),
            15 => format!(
                "Scene 15: {} rectangles with alpha + borders + radius",
                self.rectangles.len()
            ),
            16 => format!(
                "Scene 16: {} rectangles moving horizontally",
                self.rectangles.len()
            ),
            17 => format!(
                "Scene 17: {} rectangles moving in circular motion",
                self.rectangles.len()
            ),
            _ => "Unknown scene".to_string(),
        }
    }

    fn render_rectangle(
        &self,
        rect: &RectangleData,
    ) -> impl IntoElement {
        let scene = self.current_scene;
        let rect = rect.clone();

        let base_color = hsla(rect.base_hue, 0.7, 0.5, 1.0);
        let base_border_color = hsla(rect.border_base_hue, 0.8, 0.6, 1.0);

        let animation = Animation::new(Duration::from_millis(2000))
            .repeat()
            .with_easing(linear);

        let base_div = div()
            .absolute()
            .left(px(rect.x))
            .top(px(rect.y))
            .id(("rect", rect.index * SCENE_COUNT + scene));

        if !rect.changing {
            let mut rect_div = base_div
                .w(px(rect.base_width))
                .h(px(rect.base_height))
                .bg(base_color);

            if rect.base_corner_radius > 0.0 {
                rect_div = rect_div.rounded(px(rect.base_corner_radius));
            }

            if rect.base_border_width > 0.0 {
                rect_div = rect_div
                    .border_color(base_border_color)
                    .border(px(rect.base_border_width));
            }

            return rect_div.with_animation(
                ("static", rect.index),
                animation.clone(),
                move |div, _delta| div,
            );
        }

        match scene {
                0 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .with_animation(
                            ("color", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let hue = (delta + rect.base_hue) % 1.0;
                                div.bg(hsla(hue, 0.7, 0.5, 1.0))
                            },
                        )
                }

                1 | 2 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .with_animation(
                            ("color", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let hue = (delta + rect.base_hue) % 1.0;
                                div.bg(hsla(hue, 0.7, 0.5, 1.0))
                            },
                        )
                }

                3 => {
                    base_div.bg(base_color).with_animation(
                        ("size", rect.index),
                        animation.clone(),
                        move |div, delta| {
                            let scale = 1.0 + 0.3 * (delta * std::f32::consts::PI * 4.0).sin();
                            div.w(px(rect.base_width * scale))
                                .h(px(rect.base_height * scale))
                        },
                    )
                }

                4 | 5 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .bg(base_color)
                        .border(px(rect.base_border_width))
                        .with_animation(
                            ("border", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let hue = (delta + rect.border_base_hue) % 1.0;
                                div.border_color(hsla(hue, 0.8, 0.6, 1.0))
                            },
                        )
                }

                6 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .bg(base_color)
                        .border_color(base_border_color)
                        .with_animation(
                            ("border_width", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let width = rect.base_border_width
                                    + 4.0 * (delta * std::f32::consts::PI * 4.0).sin().abs();
                                div.border(px(width))
                            },
                        )
                }

                7 | 8 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .rounded(px(rect.base_corner_radius))
                        .with_animation(
                            ("color", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let hue = (delta + rect.base_hue) % 1.0;
                                div.bg(hsla(hue, 0.7, 0.5, 1.0))
                            },
                        )
                }

                9 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .bg(base_color)
                        .with_animation(
                            ("radius", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let radius = rect.base_corner_radius
                                    + 26.0 * (delta * std::f32::consts::PI * 4.0).sin().abs();
                                div.rounded(px(radius))
                            },
                        )
                }

                10 | 11 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .rounded(px(rect.base_corner_radius))
                        .border(px(rect.base_border_width))
                        .with_animation(
                            ("color", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let hue = (delta + rect.base_hue) % 1.0;
                                let border_hue = (delta + rect.border_base_hue + 0.5) % 1.0;
                                div.bg(hsla(hue, 0.7, 0.5, 1.0))
                                    .border_color(hsla(border_hue, 0.8, 0.6, 1.0))
                            },
                        )
                }

                12 => {
                    base_div.with_animation(
                        ("all", rect.index),
                        animation.clone(),
                        move |div, delta| {
                            let hue = (delta * 0.6 + rect.base_hue) % 1.0;
                            let border_hue = (delta * 0.6 + rect.border_base_hue + 0.5) % 1.0;
                            let scale = 1.0 + 0.2 * (delta * std::f32::consts::PI * 6.0).sin();
                            let radius = rect.base_corner_radius
                                + 16.0 * (delta * std::f32::consts::PI * 4.0).sin().abs();
                            let border_width = rect.base_border_width
                                + 3.0 * (delta * std::f32::consts::PI * 5.0).sin().abs();

                            div.w(px(rect.base_width * scale))
                                .h(px(rect.base_height * scale))
                                .bg(hsla(hue, 0.7, 0.5, 1.0))
                                .rounded(px(radius))
                                .border(px(border_width))
                                .border_color(hsla(border_hue, 0.8, 0.6, 1.0))
                        },
                    )
                }

                // Scene 13: Large rectangles with all properties changing
                13 => {
                    base_div.with_animation(
                        ("large_all", rect.index),
                        animation.clone(),
                        move |div, delta| {
                            let hue = (delta * 0.5 + rect.base_hue) % 1.0;
                            let border_hue = (delta * 0.5 + rect.border_base_hue + 0.5) % 1.0;
                            let scale = 1.0 + 0.15 * (delta * std::f32::consts::PI * 3.0).sin();
                            let radius = rect.base_corner_radius
                                + 20.0 * (delta * std::f32::consts::PI * 3.0).sin().abs();
                            let border_width = rect.base_border_width
                                + 4.0 * (delta * std::f32::consts::PI * 2.0).sin().abs();

                            div.w(px(rect.base_width * scale))
                                .h(px(rect.base_height * scale))
                                .bg(hsla(hue, 0.7, 0.5, 1.0))
                                .rounded(px(radius))
                                .border(px(border_width))
                                .border_color(hsla(border_hue, 0.8, 0.6, 1.0))
                        },
                    )
                }

                // Scene 14: Alpha/opacity changes
                14 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .with_animation(
                            ("alpha", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let alpha = 0.3 + 0.7 * (delta * std::f32::consts::PI * 4.0).sin().abs();
                                div.bg(hsla(rect.base_hue, 0.7, 0.5, alpha))
                            },
                        )
                }

                // Scene 15: Alpha changes with borders and radius
                15 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .rounded(px(rect.base_corner_radius))
                        .border(px(rect.base_border_width))
                        .with_animation(
                            ("alpha_border", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let alpha = 0.2 + 0.8 * (delta * std::f32::consts::PI * 3.0).sin().abs();
                                let border_alpha = 0.4 + 0.6 * (delta * std::f32::consts::PI * 3.0 + std::f32::consts::PI).sin().abs();
                                div.bg(hsla(rect.base_hue, 0.7, 0.5, alpha))
                                    .border_color(hsla(rect.border_base_hue, 0.8, 0.6, border_alpha))
                            },
                        )
                }

                // Scene 16: Horizontal movement
                16 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .rounded(px(rect.base_corner_radius))
                        .with_animation(
                            ("move_h", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let x_offset = (WINDOW_WIDTH * 0.7) * (delta * std::f32::consts::PI * 2.0).sin();
                                div.left(px(rect.x + x_offset))
                                    .bg(hsla((rect.base_hue + delta * 0.5) % 1.0, 0.7, 0.5, 1.0))
                            },
                        )
                }

                // Scene 17: Circular motion
                17 => {
                    base_div
                        .w(px(rect.base_width))
                        .h(px(rect.base_height))
                        .rounded(px(rect.base_corner_radius))
                        .border(px(rect.base_border_width))
                        .with_animation(
                            ("move_circle", rect.index),
                            animation.clone(),
                            move |div, delta| {
                                let angle = delta * std::f32::consts::PI * 2.0;
                                let radius = WINDOW_WIDTH * 0.3;
                                let x = WINDOW_CENTER_X + radius * angle.cos();
                                let y = WINDOW_CENTER_Y + radius * angle.sin();
                                div.left(px(x))
                                    .top(px(y))
                                    .bg(hsla((rect.base_hue + delta) % 1.0, 0.7, 0.5, 1.0))
                                    .border_color(hsla((rect.border_base_hue + delta + 0.5) % 1.0, 0.8, 0.6, 1.0))
                            },
                        )
                }

                _ => base_div
                    .w(px(rect.base_width))
                    .h(px(rect.base_height))
                    .bg(base_color)
                    .with_animation(
                        ("noop", rect.index),
                        animation.clone(),
                        move |div, _delta| div,
                    )
            }
    }
}

impl Render for QuadBenchmark {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;

        self.frame_times.push(frame_time);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }

        let average_frame_time: Duration =
            self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;
        if average_frame_time.as_secs_f64() > 0.0 {
            self.fps = 1.0 / average_frame_time.as_secs_f64();
        }

        let scene_elapsed = now.duration_since(self.scene_start_time).as_secs_f64();
        if scene_elapsed >= SCENE_DURATION {
            println!("\n{}", self.get_scene_description());
            println!("Average FPS: {:.1}", self.fps);

            self.current_scene = (self.current_scene + 1) % SCENE_COUNT;
            self.setup_scene();
        }

        cx.notify();

        let mut container = div().size_full().bg(rgb(0x1e1e1e)).relative();

        for rect in &self.rectangles {
            container = container.child(self.render_rectangle(rect));
        }

        container
    }
}

fn main() {
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    run_benchmark();

    #[cfg(not(all(target_os = "linux", feature = "wayland")))]
    run_benchmark_windowed();
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
fn run_benchmark() {
    use gpui::layer_shell::*;

    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                })),
                kind: WindowKind::LayerShell(LayerShellOptions {
                    namespace: "gpui-quad-benchmark".to_string(),
                    layer: Layer::Top,
                    anchor: Anchor::empty(),
                    exclusive_zone: None,
                    exclusive_edge: None,
                    margin: None,
                    keyboard_interactivity: KeyboardInteractivity::None,
                }),
                ..Default::default()
            },
            |_window, cx| {
                let benchmark = cx.new(|_| QuadBenchmark::new());
                let _ = benchmark.update(cx, |b, _| {
                    b.setup_scene();
                    println!("GPUI Quad Benchmark");
                    println!("===================");
                    println!("{}\n", b.get_scene_description());
                });
                benchmark
            },
        );
    });
}

#[cfg(not(all(target_os = "linux", feature = "wayland")))]
fn run_benchmark_windowed() {
    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                })),
                ..Default::default()
            },
            |_window, cx| {
                let benchmark = cx.new(|_| QuadBenchmark::new());
                let _ = benchmark.update(cx, |b, _| {
                    b.setup_scene();
                    println!("GPUI Quad Benchmark");
                    println!("===================");
                    println!("{}\n", b.get_scene_description());
                });
                benchmark
            },
        );
    });
}
