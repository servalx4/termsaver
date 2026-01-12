use super::Effect;
use std::io::{BufWriter, Stdout, Write};

struct Blob {
    x: f32,
    y: f32,
    vy: f32,
    radius: f32,
    temperature: f32, // 0.0 = cold (sinks), 1.0 = hot (rises)
}

pub struct LavaLampEffect {
    width: usize,
    height: usize,
    blobs: Vec<Blob>,
    field: Vec<f32>,
    time: f32,
    output_buf: Vec<u8>,
    current_color: (u8, u8, u8),
    target_color: (u8, u8, u8),
    lava_color: (u8, u8, u8), // Interpolated display color
    color_transition: f32,
}

impl Effect for LavaLampEffect {
    fn new(width: usize, height: usize) -> Self {
        // Create 8-12 blobs starting at the bottom
        let blob_count = 8 + fastrand::usize(0..5);
        let mut blobs = Vec::with_capacity(blob_count);

        for _i in 0..blob_count {
            let radius = 6.0 + fastrand::f32() * 10.0; // Radius 6-16 (can bond together)
            blobs.push(Blob {
                x: fastrand::f32() * width as f32, // Spawn randomly across full width
                y: height as f32 + radius * 2.0 - fastrand::f32() * 40.0, // Start below/at bottom
                vy: 0.0,
                radius,
                temperature: 0.8 + fastrand::f32() * 0.2, // Start hot
            });
        }

        let current_color = Self::random_lava_color();
        let target_color = Self::random_lava_color();

        Self {
            width,
            height,
            blobs,
            field: vec![0.0; width * height],
            time: 0.0,
            output_buf: Vec::with_capacity(width * height * 25),
            current_color,
            target_color,
            lava_color: current_color,
            color_transition: 0.0,
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues
        if self.time > 10000.0 {
            self.time -= 10000.0;
        }

        // Slowly transition between random colors
        self.color_transition += dt * 0.05; // Takes ~20 seconds per transition

        if self.color_transition >= 1.0 {
            // Reached target color, pick a new target
            self.current_color = self.target_color;
            self.target_color = Self::random_lava_color();
            self.color_transition = 0.0;
        }

        // Interpolate between current and target to get display color
        let t = self.color_transition;
        let r = (self.current_color.0 as f32 * (1.0 - t) + self.target_color.0 as f32 * t) as u8;
        let g = (self.current_color.1 as f32 * (1.0 - t) + self.target_color.1 as f32 * t) as u8;
        let b = (self.current_color.2 as f32 * (1.0 - t) + self.target_color.2 as f32 * t) as u8;
        self.lava_color = (r, g, b);

        let height = self.height as f32;
        let width = self.width as f32;

        // Spawn new blobs below the screen periodically
        // Limit to max 25 blobs
        if fastrand::f32() < 0.05 && self.blobs.len() < 25 {
            let radius = 6.0 + fastrand::f32() * 10.0; // Radius 6-16
            self.blobs.push(Blob {
                x: fastrand::f32() * width, // Spawn across full width
                y: height + radius * 2.0 + 10.0, // Bigger blobs spawn further below
                vy: 0.0,
                radius,
                temperature: 1.0, // Start fully heated
            });
        }

        // Update blob positions with lava lamp physics
        self.blobs.retain_mut(|blob| {
            // Temperature affects buoyancy
            // Hot blobs rise (negative vy), cold blobs sink (positive vy)
            let buoyancy = (blob.temperature - 0.5) * -40.0; // Reduced from -80.0 for slower rise

            // Apply buoyancy force
            blob.vy += buoyancy * dt;

            // Damping (thick fluid resistance)
            blob.vy *= 0.92;

            // Update position
            blob.y += blob.vy * dt * 8.0; // Reduced from 10.0 for slower movement

            // Keep blobs horizontally centered
            if blob.x < blob.radius {
                blob.x = blob.radius;
            } else if blob.x > (width - blob.radius) {
                blob.x = width - blob.radius;
            }

            // Temperature changes based on vertical position
            let relative_y = blob.y / height;

            if blob.y > height - blob.radius * 3.0 {
                // At bottom - heat up rapidly
                blob.temperature += dt * 0.5;
            } else if blob.y < 0.0 {
                // Already off-screen at top - don't cool, just keep rising
                // This ensures blobs continue off-screen to be removed
            } else {
                // Gradual temperature change based on height
                // Top half = cool down, bottom half = warm up
                if relative_y < 0.3 {
                    blob.temperature -= dt * 0.1; // Cool down slowly at top
                } else if relative_y > 0.7 {
                    blob.temperature += dt * 0.15; // Warm up at bottom
                }
            }

            // Clamp temperature
            blob.temperature = blob.temperature.clamp(0.0, 1.0);

            // Remove blobs that have gone off screen
            // Remove when blob center passes the screen edge (not waiting for full blob to disappear)
            blob.y > -blob.radius && blob.y < height + blob.radius * 3.0 + 20.0
        });

        // Calculate metaball field (simple and fast)
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let mut field_value = 0.0;

                // Sum influence from all blobs
                for blob in &self.blobs {
                    let dx = x as f32 - blob.x;
                    let dy = y as f32 - blob.y;
                    let dist_sq = dx * dx + dy * dy;

                    // Skip blobs that are too far away to contribute
                    let max_dist_sq = (blob.radius * 3.5) * (blob.radius * 3.5);
                    if dist_sq > max_dist_sq {
                        continue;
                    }

                    // Better metaball formula for smoother blending
                    let dist = dist_sq.sqrt();
                    if dist < blob.radius * 2.5 {
                        // Smooth polynomial falloff
                        let normalized = dist / (blob.radius * 2.5);
                        let influence = (1.0 - normalized).max(0.0);
                        field_value += influence * influence; // Squared for smoother falloff
                    }
                }

                self.field[idx] = field_value;
            }
        }
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H"); // Move to home

        let bg_color = crate::get_bg_color();
        let mut prev_top: (u8, u8, u8) = (255, 255, 255);
        let mut prev_bot: (u8, u8, u8) = (255, 255, 255);

        for y in (0..self.height).step_by(2) {
            for x in 0..self.width {
                let top_field = self.field[y * self.width + x];
                let bot_field = if y + 1 < self.height {
                    self.field[(y + 1) * self.width + x]
                } else {
                    0.0
                };

                // Map field value to color (threshold at 1.0)
                let top = self.field_to_color(top_field, bg_color);
                let bot = self.field_to_color(bot_field, bg_color);

                // Only emit color codes if changed
                if top != prev_top {
                    write!(self.output_buf, "\x1b[48;2;{};{};{}m", top.0, top.1, top.2)?;
                    prev_top = top;
                }
                if bot != prev_bot {
                    write!(self.output_buf, "\x1b[38;2;{};{};{}m", bot.0, bot.1, bot.2)?;
                    prev_bot = bot;
                }
                self.output_buf.extend_from_slice("▄".as_bytes());
            }
            self.output_buf.extend_from_slice(b"\x1b[0m");
            prev_top = (255, 255, 255);
            prev_bot = (255, 255, 255);
            if y + 2 < self.height {
                self.output_buf.extend_from_slice(b"\r\n");
            }
        }

        stdout.write_all(&self.output_buf)?;
        stdout.flush()?;
        Ok(())
    }
}

impl LavaLampEffect {
    fn random_lava_color() -> (u8, u8, u8) {
        // Generate vibrant lava lamp colors
        let hue = fastrand::f32(); // 0.0 to 1.0

        // Convert HSV to RGB (S=0.7-1.0, V=0.8-1.0 for vibrant colors)
        let s = 0.7 + fastrand::f32() * 0.3;
        let v = 0.8 + fastrand::f32() * 0.2;

        let h = hue * 6.0;
        let c = v * s;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 1.0 {
            (c, x, 0.0)
        } else if h < 2.0 {
            (x, c, 0.0)
        } else if h < 3.0 {
            (0.0, c, x)
        } else if h < 4.0 {
            (0.0, x, c)
        } else if h < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        (
            ((r + m) * 255.0) as u8,
            ((g + m) * 255.0) as u8,
            ((b + m) * 255.0) as u8,
        )
    }

    fn field_to_color(&self, field_value: f32, bg_color: (u8, u8, u8)) -> (u8, u8, u8) {
        // Threshold for blob visibility (adjusted for new formula)
        if field_value >= 0.4 {
            // Inside blob - use solid lava color
            self.lava_color
        } else {
            // Outside blob - use background
            bg_color
        }
    }
}
