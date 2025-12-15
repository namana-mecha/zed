use femtovg::{Color, Paint, Path};

use crate::{MonochromeSprite, platform::gl::GlRenderer};

impl GlRenderer {
    pub fn draw_monochrome_sprites(&mut self, sprites: &[MonochromeSprite]) {
        for sprite in sprites {
            let mut path = Path::new();
            path.rect(
                sprite.bounds.origin.x.0,
                sprite.bounds.origin.y.0,
                sprite.bounds.size.width.0,
                sprite.bounds.size.height.0,
            );
            let paint = Paint::color(Color::hsla(
                sprite.color.h,
                sprite.color.s,
                sprite.color.l,
                sprite.color.a,
            ));
            self.canvas.fill_path(&path, &paint);
        }
    }
}
