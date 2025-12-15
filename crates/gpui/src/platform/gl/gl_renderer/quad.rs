use femtovg::{Color, Paint, Path};

use crate::{Quad, platform::gl::GlRenderer};

impl GlRenderer {
    pub fn draw_quads(&mut self, quads: &[Quad]) {
        for quad in quads {
            // Background
            let mut path = Path::new();
            path.rounded_rect_varying(
                quad.bounds.origin.x.0,
                quad.bounds.origin.y.0,
                quad.bounds.size.width.0,
                quad.bounds.size.height.0,
                quad.corner_radii.top_left.0,
                quad.corner_radii.top_right.0,
                quad.corner_radii.bottom_right.0,
                quad.corner_radii.bottom_left.0,
            );
            let paint = match quad.background.tag {
                crate::BackgroundTag::Solid => Paint::color(Color::hsla(
                    quad.background.solid.h,
                    quad.background.solid.s,
                    quad.background.solid.l,
                    quad.background.solid.a,
                )),
                crate::BackgroundTag::LinearGradient => todo!(),
                crate::BackgroundTag::PatternSlash => todo!(),
            };
            self.canvas.fill_path(&path, &paint);

            // Border
            let mut path = Path::new();
            path.rounded_rect_varying(
                quad.bounds.origin.x.0,
                quad.bounds.origin.y.0,
                quad.bounds.size.width.0,
                quad.bounds.size.height.0,
                quad.corner_radii.top_left.0,
                quad.corner_radii.top_right.0,
                quad.corner_radii.bottom_right.0,
                quad.corner_radii.bottom_left.0,
            );
            let paint = Paint::color(Color::hsla(
                quad.border_color.h,
                quad.border_color.s,
                quad.border_color.l,
                quad.border_color.a,
            ))
            .with_line_width(quad.border_widths.left.0);
            self.canvas.stroke_path(&path, &paint);
        }
    }
}
