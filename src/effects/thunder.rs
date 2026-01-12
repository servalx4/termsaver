use super::Effect;
use std::io::{BufWriter, Stdout, Write};

const GLOW_COLORS: [(u8, u8, u8); 5] = [
    (200, 220, 255), // Bright core
    (150, 170, 200), // Inner glow
    (100, 120, 150), // Medium glow
    (60, 70, 90),    // Outer glow
    (30, 35, 50),    // Faint glow
];

#[derive(Clone)]
struct LightningSegment {
    x: f32,
    y: f32,
    end_x: f32,
    end_y: f32,
    intensity: f32,
}

struct LightningBolt {
    segments: Vec<LightningSegment>,
    age: f32,
    lifetime: f32,
    flash_intensity: f32,
    flickers: Vec<f32>, // Times when the bolt should reappear
}

impl LightningBolt {
    fn get_visibility(&self) -> f32 {
        const INITIAL_FLASH_DURATION: f32 = 0.12;
        const FLICKER_DURATION: f32 = 0.06;

        // Initial bright flash
        if self.age < INITIAL_FLASH_DURATION {
            return 1.0 - (self.age / INITIAL_FLASH_DURATION);
        }

        // Check if we're in any flicker window
        for &flicker_time in &self.flickers {
            if self.age >= flicker_time && self.age < flicker_time + FLICKER_DURATION {
                // During flicker - bright flash that fades quickly
                let t = (self.age - flicker_time) / FLICKER_DURATION;
                return 0.9 * (1.0 - t);
            }
        }

        // Between initial flash and first flicker, or between flickers - stay dark
        if let Some(&first_flicker) = self.flickers.first() {
            if self.age < first_flicker {
                return 0.0; // Dark before first flicker
            }
        }

        if self.flickers.len() > 1 {
            if self.age > self.flickers[0] + FLICKER_DURATION && self.age < self.flickers[1] {
                return 0.0; // Dark between flickers
            }
        }

        // After all flickers (or no flickers at all) - slow fade
        let fade_start = if let Some(&last_flicker) = self.flickers.last() {
            last_flicker + FLICKER_DURATION
        } else {
            INITIAL_FLASH_DURATION
        };

        if self.age > fade_start {
            let fade_progress = (self.age - fade_start) / (self.lifetime - fade_start);
            return (1.0 - fade_progress).max(0.0).powf(0.7); // Slow fade
        }

        0.0
    }

    fn new(start_x: f32, start_y: f32, width: usize, height: usize) -> Self {
        let mut segments = Vec::new();

        // Main trunk
        Self::generate_branch(
            &mut segments,
            start_x,
            start_y,
            start_x + (fastrand::f32() - 0.5) * 10.0,
            height as f32,
            1.0,
            0,
            width,
            height,
        );

        // Generate flicker times - some bolts flicker once or twice
        let mut flickers = Vec::new();
        if fastrand::f32() < 0.4 {
            // 40% chance of one flicker
            flickers.push(0.15 + fastrand::f32() * 0.1);
            if fastrand::f32() < 0.3 {
                // 30% of those get a second flicker
                flickers.push(0.35 + fastrand::f32() * 0.15);
            }
        }

        Self {
            segments,
            age: 0.0,
            lifetime: 0.4 + fastrand::f32() * 0.3, // Slower fade: 0.4-0.7 seconds
            flash_intensity: 0.8 + fastrand::f32() * 0.2,
            flickers,
        }
    }

    fn generate_branch(
        segments: &mut Vec<LightningSegment>,
        x: f32,
        y: f32,
        target_x: f32,
        target_y: f32,
        intensity: f32,
        generation: u8,
        width: usize,
        height: usize,
    ) {
        if generation > 4 || intensity < 0.2 {
            return;
        }

        // Stop if starting position is out of bounds
        if x < 0.0 || x >= width as f32 || y < 0.0 || y >= height as f32 {
            return;
        }

        let dx = target_x - x;
        let dy = target_y - y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 5.0 {
            // Only add final segment if target is in bounds
            if target_x >= 0.0 && target_x < width as f32 && target_y >= 0.0 && target_y < height as f32 {
                segments.push(LightningSegment {
                    x,
                    y,
                    end_x: target_x,
                    end_y: target_y,
                    intensity,
                });
            }
            return;
        }

        // Create multiple segments along the path (stepped leader effect)
        let num_segments = (dist / 6.0).max(2.0) as usize;
        let mut current_x = x;
        let mut current_y = y;

        for i in 0..num_segments {
            let t = (i + 1) as f32 / num_segments as f32;
            // Realistic physics: strong downward bias, less horizontal drift
            let horizontal_drift = (fastrand::f32() - 0.5) * 8.0;
            let vertical_bias = fastrand::f32() * 3.0; // Extra downward push
            let next_x = x + dx * t + horizontal_drift;
            let next_y = y + dy * t + vertical_bias;

            // Check if next position is out of bounds - if so, stop this branch
            if next_x < 0.0 || next_x >= width as f32 || next_y < 0.0 || next_y >= height as f32 {
                break;
            }

            segments.push(LightningSegment {
                x: current_x,
                y: current_y,
                end_x: next_x,
                end_y: next_y,
                intensity,
            });

            // Chance to spawn branches (reduced for narrower lightning)
            if fastrand::f32() < 0.12 && generation < 3 {
                // Realistic branching: prefer angles between 20-60 degrees, biased downward
                let side = if fastrand::bool() { 1.0 } else { -1.0 };
                let branch_angle = side * (0.3 + fastrand::f32() * 0.5); // 0.3-0.8 radians (~17-46 degrees)

                let branch_length = dist * (0.3 + fastrand::f32() * 0.4);
                let angle = dy.atan2(dx);
                let branch_target_x = next_x + (angle + branch_angle).cos() * branch_length;
                // Add extra downward bias to branch targets
                let branch_target_y = next_y + (angle + branch_angle).sin() * branch_length + branch_length * 0.3;

                Self::generate_branch(
                    segments,
                    next_x,
                    next_y,
                    branch_target_x,
                    branch_target_y,
                    intensity * 0.7,
                    generation + 1,
                    width,
                    height,
                );
            }

            current_x = next_x;
            current_y = next_y;
        }
    }
}

pub struct ThunderEffect {
    width: usize,
    height: usize,
    bolts: Vec<LightningBolt>,
    time: f32,
    next_strike_time: f32,
    output_buf: Vec<u8>,
    ambient_flash: f32,
}

impl Effect for ThunderEffect {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            bolts: Vec::new(),
            time: 0.0,
            next_strike_time: 0.3 + fastrand::f32() * 1.0,
            output_buf: Vec::with_capacity(width * height * 25),
            ambient_flash: 0.0,
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues
        if self.time > 10000.0 {
            self.time -= 10000.0;
            self.next_strike_time -= 10000.0;
        }

        // Spawn new lightning bolts
        if self.time >= self.next_strike_time {
            let x = fastrand::usize(10..self.width - 10) as f32;
            let y = 0.0;

            self.bolts.push(LightningBolt::new(x, y, self.width, self.height));

            // Sometimes spawn multiple strikes in different locations
            if fastrand::f32() < 0.25 {
                // Spawn at a completely different location
                let x2 = fastrand::usize(10..self.width - 10) as f32;
                self.bolts.push(LightningBolt::new(x2, y, self.width, self.height));

                // Rare triple strike
                if fastrand::f32() < 0.15 {
                    let x3 = fastrand::usize(10..self.width - 10) as f32;
                    self.bolts.push(LightningBolt::new(x3, y, self.width, self.height));
                }
            }

            self.next_strike_time = self.time + 0.5 + fastrand::f32() * 2.0;
        }

        // Update bolts and calculate ambient flash
        self.ambient_flash = 0.0;
        self.bolts.retain_mut(|bolt| {
            bolt.age += dt;
            let alive = bolt.age < bolt.lifetime;

            if alive {
                // Flash based on visibility
                let visibility = bolt.get_visibility();
                let flash = bolt.flash_intensity * visibility;
                self.ambient_flash = self.ambient_flash.max(flash);
            }

            alive
        });
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H"); // Move to home

        // Create buffer for lightning glow
        let mut glow_buffer = vec![0.0f32; self.width * self.height];

        // Draw all lightning segments into glow buffer
        for bolt in &self.bolts {
            let fade = bolt.get_visibility();

            for segment in &bolt.segments {
                // Bresenham-like line drawing
                let x0 = segment.x as i32;
                let y0 = segment.y as i32;
                let x1 = segment.end_x as i32;
                let y1 = segment.end_y as i32;

                let dx = (x1 - x0).abs();
                let dy = (y1 - y0).abs();
                let sx = if x0 < x1 { 1 } else { -1 };
                let sy = if y0 < y1 { 1 } else { -1 };
                let mut err = dx - dy;

                let mut x = x0;
                let mut y = y0;

                let intensity = segment.intensity * fade;

                loop {
                    // Draw main bolt and glow
                    for gy in (y - 3).max(0)..=(y + 3).min(self.height as i32 - 1) {
                        for gx in (x - 3).max(0)..=(x + 3).min(self.width as i32 - 1) {
                            let dist = ((gx - x).pow(2) + (gy - y).pow(2)) as f32;
                            let glow = (intensity * 5.0 / (dist + 1.0)).min(5.0);
                            let idx = gy as usize * self.width + gx as usize;
                            glow_buffer[idx] = glow_buffer[idx].max(glow);
                        }
                    }

                    if x == x1 && y == y1 {
                        break;
                    }

                    let e2 = 2 * err;
                    if e2 > -dy {
                        err -= dy;
                        x += sx;
                    }
                    if e2 < dx {
                        err += dx;
                        y += sy;
                    }
                }
            }
        }

        // Calculate background color based on ambient flash
        let base_bg = crate::get_bg_color();
        let bg = if self.ambient_flash > 0.01 {
            let flash_brightness = (self.ambient_flash * 80.0) as u8;
            (
                base_bg.0.saturating_add(flash_brightness),
                base_bg.1.saturating_add(flash_brightness),
                base_bg.2.saturating_add((flash_brightness as f32 * 1.2) as u8),
            )
        } else {
            base_bg
        };

        let mut prev_color: (u8, u8, u8) = (255, 255, 255);

        // Render using half-blocks
        for y in (0..self.height).step_by(2) {
            for x in 0..self.width {
                let top_glow = glow_buffer[y * self.width + x];
                let bot_glow = if y + 1 < self.height {
                    glow_buffer[(y + 1) * self.width + x]
                } else {
                    0.0
                };

                let top_color = Self::glow_to_color(top_glow, bg);
                let bot_color = Self::glow_to_color(bot_glow, bg);

                // Only emit color codes if changed
                if top_color != prev_color {
                    write!(
                        self.output_buf,
                        "\x1b[48;2;{};{};{}m",
                        top_color.0, top_color.1, top_color.2
                    )?;
                }
                write!(
                    self.output_buf,
                    "\x1b[38;2;{};{};{}m",
                    bot_color.0, bot_color.1, bot_color.2
                )?;
                prev_color = top_color;

                self.output_buf.extend_from_slice("▄".as_bytes());
            }
            self.output_buf.extend_from_slice(b"\x1b[0m");
            prev_color = (255, 255, 255);
            if y + 2 < self.height {
                self.output_buf.extend_from_slice(b"\r\n");
            }
        }

        stdout.write_all(&self.output_buf)?;
        stdout.flush()?;
        Ok(())
    }
}

impl ThunderEffect {
    fn glow_to_color(glow: f32, bg: (u8, u8, u8)) -> (u8, u8, u8) {
        if glow < 0.1 {
            return bg;
        }

        let idx = (glow as usize).min(GLOW_COLORS.len() - 1);
        let color = GLOW_COLORS[idx];

        // Blend with background
        let blend = (glow.fract()).min(1.0);
        (
            (bg.0 as f32 * (1.0 - blend) + color.0 as f32 * blend) as u8,
            (bg.1 as f32 * (1.0 - blend) + color.1 as f32 * blend) as u8,
            (bg.2 as f32 * (1.0 - blend) + color.2 as f32 * blend) as u8,
        )
    }
}
