use gpui::*;
use std::time::{Duration, Instant};

const BENCHMARK_DURATION: Duration = Duration::from_secs(10);
const WARMUP_FRAMES: usize = 60;

struct BenchmarkApp {
    current_bench: usize,
    benchmarks: Vec<BenchmarkSpec>,
    frame_times: Vec<Duration>,
    start_time: Option<Instant>,
    last_frame_time: Option<Instant>,
    warmup_frames: usize,
    total_frames: usize,
    frame_number: usize,
}

struct BenchmarkSpec {
    name: &'static str,
    description: &'static str,
    renderer: fn(usize) -> AnyElement,
}

// Benchmark 1: Small Area Animation - Single element moving
fn small_area_animation(frame: usize) -> AnyElement {
    let t = (frame as f32 * 0.05) % (2.0 * std::f32::consts::PI);
    let x = 50.0 + t.sin() * 300.0 + 300.0;
    let y = 50.0 + t.cos() * 200.0 + 200.0;

    div()
        .flex()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .child(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .size(px(60.0))
                .bg(rgb(0x00aaff))
                .rounded(px(8.0))
        )
        .into_any()
}

// Benchmark 2: Multiple Elements Animation
fn multiple_elements(frame: usize) -> AnyElement {
    let mut container = div()
        .flex()
        .flex_wrap()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .gap(px(8.0))
        .p(px(20.0));

    for i in 0..30 {
        let t = (frame as f32 + i as f32 * 10.0) * 0.03;
        let scale = 1.0 + (t.sin() * 0.3);
        let size = 50.0 * scale;

        let hue = ((i as f32 * 12.0 + frame as f32 * 2.0) % 360.0) / 360.0;
        let (r, g, b) = hsv_to_rgb(hue * 360.0, 0.8, 0.9);
        let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

        container = container.child(
            div()
                .size(px(size))
                .bg(color)
                .rounded(px(8.0))
        );
    }

    container.into_any()
}

// Benchmark 3: Single Element Over Static Background
fn single_over_background(frame: usize) -> AnyElement {
    let mut container = div()
        .flex()
        .flex_wrap()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .gap(px(2.0))
        .p(px(2.0))
        .relative();

    // Static background: 400 small elements
    for i in 0..400 {
        let hue = (i as f32 * 0.9) % 360.0;
        let (r, g, b) = hsv_to_rgb(hue, 0.4, 0.5);
        let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

        container = container.child(
            div()
                .size(px(22.0))
                .bg(color)
                .rounded(px(2.0))
        );
    }

    // Single moving element
    let t = (frame as f32 * 0.04) % (2.0 * std::f32::consts::PI);
    let x = 400.0 + t.cos() * 350.0;
    let y = 300.0 + t.sin() * 250.0;

    container = container.child(
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .size(px(100.0))
            .bg(rgb(0xff3333))
            .rounded(px(50.0))
            .border_4()
            .border_color(rgb(0xffffff))
    );

    container.into_any()
}

// Benchmark 4: Wave Animation
fn wave_animation(frame: usize) -> AnyElement {
    let mut container = div()
        .flex()
        .flex_row()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .gap(px(4.0))
        .p(px(20.0))
        .items_end()
        .justify_center();

    for i in 0..20 {
        let offset = (frame as f32 * 0.1 + i as f32 * 0.3).sin();
        let height = 100.0 + offset * 150.0;

        let hue = (i as f32 * 18.0) % 360.0;
        let (r, g, b) = hsv_to_rgb(hue, 0.7, 0.8);
        let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

        container = container.child(
            div()
                .w(px(35.0))
                .h(px(height))
                .bg(color)
                .rounded(px(6.0))
        );
    }

    container.into_any()
}

// Benchmark 5: Pulsing Grid
fn pulsing_grid(frame: usize) -> AnyElement {
    let mut container = div()
        .flex()
        .flex_wrap()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .gap(px(4.0))
        .p(px(15.0));

    for i in 0..120 {
        let row = i / 12;
        let col = i % 12;
        let t = (frame as f32 * 0.08 + row as f32 * 0.2 + col as f32 * 0.15).sin();
        let brightness = 0.3 + t * 0.5;

        let base_hue = (i as f32 * 3.0) % 360.0;
        let (r, g, b) = hsv_to_rgb(base_hue, 0.7, brightness);
        let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

        container = container.child(
            div()
                .size(px(55.0))
                .bg(color)
                .rounded(px(8.0))
        );
    }

    container.into_any()
}

// Benchmark 6: Rotating Circles
fn rotating_circles(frame: usize) -> AnyElement {
    let mut container = div()
        .flex()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .items_center()
        .justify_center()
        .relative();

    let num_circles = 15;
    for i in 0..num_circles {
        let base_angle = (i as f32 / num_circles as f32) * 2.0 * std::f32::consts::PI;
        let rotation = frame as f32 * 0.02;
        let angle = base_angle + rotation;

        let radius = 220.0;
        let x = 512.0 + angle.cos() * radius - 30.0;
        let y = 384.0 + angle.sin() * radius - 30.0;

        let hue = (i as f32 * (360.0 / num_circles as f32)) % 360.0;
        let (r, g, b) = hsv_to_rgb(hue, 0.9, 0.9);
        let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

        container = container.child(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .size(px(60.0))
                .bg(color)
                .rounded(px(30.0))
                .opacity(0.8)
        );
    }

    container.into_any()
}

// Benchmark 7: Complex Mixed Scene
fn mixed_scene(frame: usize) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(rgb(0x1e1e1e))
        .gap(px(10.0))
        .p(px(15.0))
        // Animated header
        .child({
            let mut header = div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .h(px(100.0));

            for i in 0..3 {
                let t = (frame as f32 * 0.05 + i as f32 * 1.0).sin();
                let height = 80.0 + t * 15.0;
                let hue = (120.0 * i as f32) % 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 0.7, 0.8);
                let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

                header = header.child(
                    div()
                        .flex_1()
                        .h(px(height))
                        .bg(color)
                        .rounded(px(8.0))
                );
            }

            header
        })
        // Text section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(div().text_color(rgb(0xffffff)).text_size(px(18.0)).child("Rendering Benchmark Active"))
                .child(div().text_color(rgb(0xaaaaaa)).text_size(px(14.0)).child("Testing damage tracking and animation performance"))
        )
        // Animated grid
        .child({
            let mut grid = div()
                .flex()
                .flex_wrap()
                .gap(px(6.0))
                .flex_1();

            for i in 0..25 {
                let t = (frame as f32 * 0.06 + i as f32 * 0.2).sin();
                let size = 50.0 + t * 10.0;
                let hue = (i as f32 * 14.4 + frame as f32) % 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 0.8, 0.85);
                let color = rgb(((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32);

                grid = grid.child(
                    div()
                        .size(px(size))
                        .bg(color)
                        .rounded(px(8.0))
                );
            }

            grid
        })
        .into_any()
}

impl Render for BenchmarkApp {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Request next frame to keep animating
        window.request_animation_frame();

        // Track frame timing
        let now = Instant::now();
        if let Some(start) = self.start_time {
            let elapsed = now.duration_since(start);

            // Calculate time since last frame
            if let Some(last_frame) = self.last_frame_time {
                let frame_time = now.duration_since(last_frame);

                if self.warmup_frames < WARMUP_FRAMES {
                    self.warmup_frames += 1;
                } else {
                    self.frame_times.push(frame_time);
                }
            }

            self.last_frame_time = Some(now);

            // Check if benchmark is complete
            if elapsed >= BENCHMARK_DURATION && self.warmup_frames >= WARMUP_FRAMES {
                self.finish_current_benchmark();

                if self.current_bench >= self.benchmarks.len() {
                    println!("\n╔══════════════════════════════════════════════════════════════╗");
                    println!("║              All Benchmarks Complete!                       ║");
                    println!("╚══════════════════════════════════════════════════════════════╝\n");
                    std::process::exit(0);
                } else {
                    self.start_time = Some(Instant::now());
                    self.last_frame_time = None;
                    self.frame_times.clear();
                    self.warmup_frames = 0;
                    self.total_frames = 0;

                    let bench = &self.benchmarks[self.current_bench];
                    println!("\n=== Starting: {} ===", bench.name);
                    println!("{}", bench.description);
                    println!("Running for {} seconds...\n", BENCHMARK_DURATION.as_secs());
                }
            }
        } else {
            self.start_time = Some(now);
        }

        self.total_frames += 1;
        self.frame_number += 1;

        // Render current benchmark
        let content = if self.current_bench < self.benchmarks.len() {
            let bench = &self.benchmarks[self.current_bench];
            (bench.renderer)(self.frame_number)
        } else {
            div().size_full().bg(rgb(0x000000)).into_any()
        };

        // Wrap with progress overlay
        let mut root = div()
            .flex()
            .size_full()
            .child(content);

        // Add progress indicator
        if let Some(start) = self.start_time {
            let elapsed = Instant::now().duration_since(start);
            let progress = (elapsed.as_secs_f32() / BENCHMARK_DURATION.as_secs_f32()).min(1.0);
            let percent = (progress * 100.0) as u32;

            let fps = if self.frame_times.len() > 10 {
                let recent: Vec<_> = self.frame_times.iter().rev().take(60).copied().collect();
                let avg_time: Duration = recent.iter().sum::<Duration>() / recent.len() as u32;
                1.0 / avg_time.as_secs_f64()
            } else {
                0.0
            };

            root = root.child(
                div()
                    .absolute()
                    .top(px(15.0))
                    .right(px(15.0))
                    .p(px(15.0))
                    .bg(rgba(0x000000dd))
                    .rounded(px(10.0))
                    .border_1()
                    .border_color(rgba(0xffffff33))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(16.0))
                                    .child(format!("Benchmark {}/{}", self.current_bench + 1, self.benchmarks.len()))
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x00ff00))
                                    .text_size(px(14.0))
                                    .child(format!("Progress: {}%", percent))
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x00aaff))
                                    .text_size(px(14.0))
                                    .child(format!("FPS: {:.1}", fps))
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xaaaaaa))
                                    .text_size(px(12.0))
                                    .child(format!("Frames: {}", self.total_frames))
                            )
                    )
            );
        }

        root
    }
}

impl BenchmarkApp {
    fn new() -> Self {
        let benchmarks = vec![
            BenchmarkSpec {
                name: "Small Area Animation",
                description: "Single element moving smoothly - tests minimal damage region tracking",
                renderer: small_area_animation,
            },
            BenchmarkSpec {
                name: "Multiple Elements",
                description: "30 independently scaling elements - tests multiple damage regions",
                renderer: multiple_elements,
            },
            BenchmarkSpec {
                name: "Single Over Background",
                description: "One moving element over 400 static elements - tests selective invalidation",
                renderer: single_over_background,
            },
            BenchmarkSpec {
                name: "Wave Animation",
                description: "20 bars in wave pattern - tests synchronized animations",
                renderer: wave_animation,
            },
            BenchmarkSpec {
                name: "Pulsing Grid",
                description: "120 pulsing elements - tests full-grid color updates",
                renderer: pulsing_grid,
            },
            BenchmarkSpec {
                name: "Rotating Circles",
                description: "15 elements in orbital motion - tests position-based animations",
                renderer: rotating_circles,
            },
            BenchmarkSpec {
                name: "Mixed Scene",
                description: "Complex scene with header, text, and animated grid",
                renderer: mixed_scene,
            },
        ];

        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║      GPUI Rendering & Animation Benchmark Suite             ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");
        println!("Testing damage tracking, layout, and animation performance");
        println!("Running {} benchmarks, each for {} seconds", benchmarks.len(), BENCHMARK_DURATION.as_secs());
        println!("Watch the window to see the animations!\n");

        let first_bench = &benchmarks[0];
        println!("=== Starting: {} ===", first_bench.name);
        println!("{}", first_bench.description);
        println!("Running for {} seconds...\n", BENCHMARK_DURATION.as_secs());

        Self {
            current_bench: 0,
            benchmarks,
            frame_times: Vec::new(),
            start_time: None,
            last_frame_time: None,
            warmup_frames: 0,
            total_frames: 0,
            frame_number: 0,
        }
    }

    fn finish_current_benchmark(&mut self) {
        if self.frame_times.is_empty() {
            return;
        }

        let total_time: Duration = self.frame_times.iter().sum();
        let frame_count = self.frame_times.len();

        let avg_frame_time = total_time.as_secs_f64() / frame_count as f64;
        let fps = 1.0 / avg_frame_time;

        // Calculate percentiles
        let mut sorted_times = self.frame_times.clone();
        sorted_times.sort();

        let p50 = sorted_times[frame_count / 2];
        let p95 = sorted_times[(frame_count * 95) / 100];
        let p99 = sorted_times[(frame_count * 99) / 100];
        let min = sorted_times[0];
        let max = sorted_times[frame_count - 1];

        println!("\n--- Results ---");
        println!("Total Frames:  {}", self.total_frames);
        println!("Measured:      {} (after {} warmup frames)", frame_count, WARMUP_FRAMES);
        println!("Average FPS:   {:.2}", fps);
        println!("Avg Frame:     {:.3}ms", avg_frame_time * 1000.0);
        println!("Min Frame:     {:.3}ms", min.as_secs_f64() * 1000.0);
        println!("P50 Frame:     {:.3}ms", p50.as_secs_f64() * 1000.0);
        println!("P95 Frame:     {:.3}ms", p95.as_secs_f64() * 1000.0);
        println!("P99 Frame:     {:.3}ms", p99.as_secs_f64() * 1000.0);
        println!("Max Frame:     {:.3}ms", max.as_secs_f64() * 1000.0);

        // Calculate score (higher is better - based on FPS)
        let score = fps * 10.0;
        println!("\nScore:         {:.0} points", score);

        self.current_bench += 1;
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());

    let (r1, g1, b1) = match h_prime as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let m = v - c;
    (r1 + m, g1 + m, b1 + m)
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(100.0), px(100.0)),
                    size: size(px(1024.0), px(768.0)),
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("GPUI Benchmark - Watch the Animations!".into()),
                    ..Default::default()
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| BenchmarkApp::new()),
        )
        .unwrap();
    });
}
