use glow::HasContext;
use std::cell::RefCell;
use std::rc::Rc;

use crate::{MonochromeSprite, platform::gl::GlRenderer};

// Simple vertex shader for textured quads
const VERTEX_SHADER: &str = r#"
#version 330 core
layout (location = 0) in vec2 position;
layout (location = 1) in vec2 texCoord;

out vec2 TexCoord;

uniform vec2 screenSize;
uniform vec4 spriteBounds; // x, y, width, height

void main() {
    // Transform vertex position to sprite bounds
    vec2 pos = spriteBounds.xy + position * spriteBounds.zw;

    // Convert to normalized device coordinates (-1 to 1)
    vec2 ndc = (pos / screenSize) * 2.0 - 1.0;
    ndc.y = -ndc.y; // Flip Y axis

    gl_Position = vec4(ndc, 0.0, 1.0);
    TexCoord = texCoord;
}
"#;

// Fragment shader that samples grayscale texture and applies color
const FRAGMENT_SHADER: &str = r#"
#version 330 core
in vec2 TexCoord;
out vec4 FragColor;

uniform sampler2D texture1;
uniform vec4 color;

void main() {
    // Sample the red channel (grayscale data)
    float alpha = texture(texture1, TexCoord).r;

    // Apply color with alpha from texture
    FragColor = vec4(color.rgb, color.a * alpha);
}
"#;

pub struct MonochromeSpriteRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
}

impl MonochromeSpriteRenderer {
    pub fn new(gl: &Rc<glow::Context>) -> Result<Self, String> {
        unsafe {
            // Compile shaders
            let vertex_shader = gl.create_shader(glow::VERTEX_SHADER)?;
            gl.shader_source(vertex_shader, VERTEX_SHADER);
            gl.compile_shader(vertex_shader);
            if !gl.get_shader_compile_status(vertex_shader) {
                return Err(gl.get_shader_info_log(vertex_shader));
            }

            let fragment_shader = gl.create_shader(glow::FRAGMENT_SHADER)?;
            gl.shader_source(fragment_shader, FRAGMENT_SHADER);
            gl.compile_shader(fragment_shader);
            if !gl.get_shader_compile_status(fragment_shader) {
                return Err(gl.get_shader_info_log(fragment_shader));
            }

            // Link program
            let program = gl.create_program()?;
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }

            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            // Create VAO and buffers
            let vao = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao));

            // Vertex data: position (x, y) and texcoord (u, v)
            #[rustfmt::skip]
            let vertices: [f32; 16] = [
                // positions  // texcoords
                0.0, 0.0,     0.0, 0.0,  // bottom-left
                1.0, 0.0,     1.0, 0.0,  // bottom-right
                1.0, 1.0,     1.0, 1.0,  // top-right
                0.0, 1.0,     0.0, 1.0,  // top-left
            ];

            let vbo = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            // Indices for two triangles
            let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
            let ebo = gl.create_buffer()?;
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(&indices),
                glow::STATIC_DRAW,
            );

            // Position attribute
            gl.vertex_attrib_pointer_f32(
                0,
                2,
                glow::FLOAT,
                false,
                4 * std::mem::size_of::<f32>() as i32,
                0,
            );
            gl.enable_vertex_attrib_array(0);

            // TexCoord attribute
            gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                4 * std::mem::size_of::<f32>() as i32,
                2 * std::mem::size_of::<f32>() as i32,
            );
            gl.enable_vertex_attrib_array(1);

            gl.bind_vertex_array(None);

            Ok(Self {
                program,
                vao,
                vbo,
                ebo,
            })
        }
    }

    pub fn render(
        &self,
        gl: &Rc<glow::Context>,
        sprite: &MonochromeSprite,
        texture: glow::Texture,
        screen_width: f32,
        screen_height: f32,
    ) {
        unsafe {
            gl.use_program(Some(self.program));

            // Set uniforms
            if let Some(loc) = gl.get_uniform_location(self.program, "screenSize") {
                gl.uniform_2_f32(Some(&loc), screen_width, screen_height);
            }

            if let Some(loc) = gl.get_uniform_location(self.program, "spriteBounds") {
                gl.uniform_4_f32(
                    Some(&loc),
                    sprite.bounds.origin.x.0 as f32,
                    sprite.bounds.origin.y.0 as f32,
                    sprite.bounds.size.width.0 as f32,
                    sprite.bounds.size.height.0 as f32,
                );
            }

            // Convert HSLA to RGBA
            let color = hsla_to_rgba(
                sprite.color.h,
                sprite.color.s,
                sprite.color.l,
                sprite.color.a,
            );
            if let Some(loc) = gl.get_uniform_location(self.program, "color") {
                gl.uniform_4_f32(Some(&loc), color.0, color.1, color.2, color.3);
            }

            // Bind texture
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            if let Some(loc) = gl.get_uniform_location(self.program, "texture1") {
                gl.uniform_1_i32(Some(&loc), 0);
            }

            // Draw
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0);
            gl.bind_vertex_array(None);
        }
    }
}

impl Drop for MonochromeSpriteRenderer {
    fn drop(&mut self) {
    }
}

fn hsla_to_rgba(h: f32, s: f32, l: f32, a: f32) -> (f32, f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 1.0 / 6.0 {
        (c, x, 0.0)
    } else if h < 2.0 / 6.0 {
        (x, c, 0.0)
    } else if h < 3.0 / 6.0 {
        (0.0, c, x)
    } else if h < 4.0 / 6.0 {
        (0.0, x, c)
    } else if h < 5.0 / 6.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    ((r + m) * a, (g + m) * a, (b + m) * a, a)
}

thread_local! {
    static SPRITE_RENDERER: RefCell<Option<MonochromeSpriteRenderer>> = RefCell::new(None);
}

impl GlRenderer {
    pub fn draw_monochrome_sprites(&mut self, sprites: &[MonochromeSprite]) {
        if sprites.is_empty() {
            return;
        }

        // Initialize sprite renderer if needed
        SPRITE_RENDERER.with(|renderer_cell| {
            let mut renderer_opt = renderer_cell.borrow_mut();
            if renderer_opt.is_none() {
                match MonochromeSpriteRenderer::new(&self.gl) {
                    Ok(renderer) => *renderer_opt = Some(renderer),
                    Err(e) => {
                        eprintln!("Failed to create sprite renderer: {}", e);
                    }
                }
            }
        });

        // Flush femtovg before custom GL rendering
        self.canvas.flush();

        SPRITE_RENDERER.with(|renderer_cell| {
            let renderer_opt = renderer_cell.borrow();
            if let Some(renderer) = renderer_opt.as_ref() {
                unsafe {
                    let gl = &self.gl;

                    // Get screen size
                    let screen_width = self.canvas.width();
                    let screen_height = self.canvas.height();

                    // Save GL state
                    let old_blend_enabled = gl.is_enabled(glow::BLEND);

                    // Enable blending
                    gl.enable(glow::BLEND);
                    gl.blend_func_separate(
                        glow::SRC_ALPHA,
                        glow::ONE_MINUS_SRC_ALPHA,
                        glow::ONE,
                        glow::ONE_MINUS_SRC_ALPHA,
                    );

                    for sprite in sprites {
                        let atlas = self.atlas.clone();

                        if let Some((data, tile_bounds)) =
                            atlas.get_texture_data(sprite.tile.texture_id)
                        {
                            let width = tile_bounds.size.width.0 as i32;
                            let height = tile_bounds.size.height.0 as i32;

                            let expected_size = (width * height) as usize;
                            if data.len() != expected_size {
                                eprintln!(
                                    "Data size mismatch: got {}, expected {}",
                                    data.len(),
                                    expected_size
                                );
                                continue;
                            }

                            // Create texture
                            let texture = gl.create_texture().unwrap();
                            gl.bind_texture(glow::TEXTURE_2D, Some(texture));

                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_MIN_FILTER,
                                glow::LINEAR as i32,
                            );
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_MAG_FILTER,
                                glow::LINEAR as i32,
                            );
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_WRAP_S,
                                glow::CLAMP_TO_EDGE as i32,
                            );
                            gl.tex_parameter_i32(
                                glow::TEXTURE_2D,
                                glow::TEXTURE_WRAP_T,
                                glow::CLAMP_TO_EDGE as i32,
                            );

                            // Set pixel unpack alignment to 1 byte (critical for single-channel textures)
                            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

                            // Upload grayscale data as RED channel (single-channel texture)
                            gl.tex_image_2d(
                                glow::TEXTURE_2D,
                                0,
                                glow::R8 as i32,
                                width,
                                height,
                                0,
                                glow::RED,
                                glow::UNSIGNED_BYTE,
                                glow::PixelUnpackData::Slice(Some(data.as_slice())),
                            );

                            // Reset alignment to default
                            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);

                            // Render sprite
                            renderer.render(
                                gl,
                                sprite,
                                texture,
                                screen_width as f32,
                                screen_height as f32,
                            );

                            // Cleanup texture
                            gl.delete_texture(texture);
                        }
                    }

                    // Restore GL state
                    if !old_blend_enabled {
                        gl.disable(glow::BLEND);
                    }

                    // Reset any other GL state that might affect femtovg
                    gl.bind_vertex_array(None);
                    gl.use_program(None);
                }
            }
        });
    }
}
