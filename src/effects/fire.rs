use super::Effect;
use noise::{NoiseFn, Perlin};
use std::io::{BufWriter, Stdout, Write};

const PALETTE: [(u8, u8, u8); 37] = [
    (0x07, 0x07, 0x07), (0x1F, 0x07, 0x07), (0x2F, 0x0F, 0x07), (0x47, 0x0F, 0x07),
    (0x57, 0x17, 0x07), (0x67, 0x1F, 0x07), (0x77, 0x1F, 0x07), (0x8F, 0x27, 0x07),
    (0x9F, 0x2F, 0x07), (0xAF, 0x3F, 0x07), (0xBF, 0x47, 0x07), (0xC7, 0x47, 0x07),
    (0xDF, 0x4F, 0x07), (0xDF, 0x57, 0x07), (0xDF, 0x57, 0x07), (0xD7, 0x5F, 0x07),
    (0xD7, 0x67, 0x0F), (0xCF, 0x6F, 0x0F), (0xCF, 0x77, 0x0F), (0xCF, 0x7F, 0x0F),
    (0xCF, 0x87, 0x17), (0xC7, 0x87, 0x17), (0xC7, 0x8F, 0x17), (0xC7, 0x97, 0x1F),
    (0xBF, 0x9F, 0x1F), (0xBF, 0x9F, 0x1F), (0xBF, 0xA7, 0x27), (0xBF, 0xA7, 0x27),
    (0xBF, 0xAF, 0x2F), (0xB7, 0xAF, 0x2F), (0xB7, 0xB7, 0x2F), (0xB7, 0xB7, 0x37),
    (0xCF, 0xCF, 0x6F), (0xDF, 0xDF, 0x9F), (0xEF, 0xEF, 0xC7), (0xFF, 0xFF, 0xFF),
    (0xFF, 0xFF, 0xFF),
];

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    brightness: u8,
}

pub struct FireEffect {
    width: usize,
    height: usize,
    buffer: Vec<f32>,
    sparks: Vec<Spark>,
    perlin: Perlin,
    turb_perlin: Perlin,
    time: f32,
    wind: f32,
    height_cache: Vec<f32>,
    output_buf: Vec<u8>,
    decay_scale: f32,
}

impl Effect for FireEffect {
    fn new(width: usize, height: usize) -> Self {
        let perlin = Perlin::new(fastrand::u32(0..1000));
        let turb_perlin = Perlin::new(fastrand::u32(0..1000));

        // Scale decay based on terminal height (56 rows * 2 = 112 is baseline)
        // Taller terminals = less decay = flames reach higher
        let decay_scale = 112.0 / height as f32;

        Self {
            width,
            height,
            buffer: vec![0.0; width * height],
            sparks: Vec::with_capacity(64),
            perlin,
            turb_perlin,
            time: 0.0,
            wind: 0.0,
            height_cache: vec![0.0; width],
            output_buf: Vec::with_capacity(width * height * 25),
            decay_scale,
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues with large numbers
        if self.time > 10000.0 {
            self.time -= 10000.0;
        }

        // Update wind
        self.wind = (self.time * 0.7).sin() * 1.5
            + (self.time * 1.3).sin() * 0.8
            + (self.time * 2.1).sin() * 0.4;

        // Cache height noise per column (only changes with time)
        for x in 0..self.width {
            let height_noise = self.perlin.get([x as f64 * 0.02, self.time as f64 * 0.3]) as f32;
            self.height_cache[x] = 0.6 + height_noise * 0.5;
        }

        // Update fuel source
        let base_row = self.height - 1;
        for x in 0..self.width {
            let noise_val = self.perlin.get([x as f64 * 0.05, self.time as f64 * 0.8]) as f32;
            let fuel = 28.0 + noise_val * 6.0 + fastrand::f32() * 3.0;
            self.buffer[base_row * self.width + x] = fuel;
        }

        self.spread_fire();

        // Spawn sparks
        if fastrand::f32() < 0.2 {
            let x = fastrand::usize(0..self.width) as f32;
            let intensity = self.buffer[(self.height - 2) * self.width + x as usize];
            if intensity > 25.0 {
                self.sparks.push(Spark {
                    x,
                    y: (self.height - 2) as f32,
                    vx: fastrand::f32() - 0.5 + self.wind * 0.2,
                    vy: -(fastrand::f32() * 1.5 + 1.0),
                    life: 1.0,
                    brightness: fastrand::u8(18..24),
                });
            }
        }

        // Update sparks
        let wind = self.wind;
        let w = self.width as f32;
        self.sparks.retain_mut(|spark| {
            spark.x += spark.vx + wind * 0.1;
            spark.y += spark.vy;
            spark.vy += 0.05;
            spark.vx += fastrand::f32() * 0.3 - 0.15;
            spark.life -= dt * 0.8;

            spark.life > 0.0 && spark.x >= 0.0 && spark.x < w && spark.y >= 0.0
        });
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H"); // Move to home

        let bg_color = crate::get_bg_color();
        let mut prev_top: (u8, u8, u8) = (255, 255, 255);
        let mut prev_bot: (u8, u8, u8) = (255, 255, 255);

        for y in (0..self.height).step_by(2) {
            for x in 0..self.width {
                let mut top_intensity = self.buffer[y * self.width + x];
                let mut bot_intensity = if y + 1 < self.height {
                    self.buffer[(y + 1) * self.width + x]
                } else {
                    36.0
                };

                // Check sparks
                for spark in &self.sparks {
                    let sy = spark.y as usize;
                    let sx = spark.x as usize;
                    if sx == x {
                        if sy == y {
                            top_intensity = top_intensity.max(spark.brightness as f32 * spark.life);
                        } else if sy == y + 1 {
                            bot_intensity = bot_intensity.max(spark.brightness as f32 * spark.life);
                        }
                    }
                }

                let top_idx = (top_intensity as usize).min(36);
                let bot_idx = (bot_intensity as usize).min(36);

                let top = Self::blend_with_bg(PALETTE[top_idx], bg_color, top_idx);
                let bot = Self::blend_with_bg(PALETTE[bot_idx], bg_color, bot_idx);

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

impl FireEffect {
    fn blend_with_bg(palette_color: (u8, u8, u8), bg_color: (u8, u8, u8), index: usize) -> (u8, u8, u8) {
        // Blend lower palette indices (cooler/background areas) with bg_color
        // Index 0-5 = mostly background, 6+ = pure fire colors
        if index <= 5 {
            let blend_factor = index as f32 / 5.0; // 0.0 at index 0, 1.0 at index 5
            (
                (bg_color.0 as f32 * (1.0 - blend_factor) + palette_color.0 as f32 * blend_factor) as u8,
                (bg_color.1 as f32 * (1.0 - blend_factor) + palette_color.1 as f32 * blend_factor) as u8,
                (bg_color.2 as f32 * (1.0 - blend_factor) + palette_color.2 as f32 * blend_factor) as u8,
            )
        } else {
            palette_color
        }
    }

    fn spread_fire(&mut self) {
        let width = self.width;
        let time = self.time;
        let wind = self.wind;

        for y in 1..self.height {
            for x in 0..width {
                let src = y * width + x;
                let intensity = self.buffer[src];

                // Simplified turbulence - sample less often
                let turb = if (x + y) % 2 == 0 {
                    self.turb_perlin.get([
                        x as f64 * 0.03,
                        y as f64 * 0.03,
                        time as f64 * 0.5,
                    ]) as f32
                } else {
                    0.0
                };

                let drift = wind * 0.4 + turb * 2.0 + fastrand::f32() * 2.0 - 1.0;
                let dst_x = (x as f32 + drift).clamp(0.0, (width - 1) as f32) as usize;
                let dst = (y - 1) * width + dst_x;

                let height_factor = self.height_cache[x];
                let heat_decay = 1.0 + intensity * 0.03;
                let base_decay = fastrand::f32() * 1.2 + 0.3;
                let decay = base_decay * height_factor * heat_decay * self.decay_scale;

                self.buffer[dst] = (self.buffer[src] - decay).max(0.0);
            }
        }
    }
}
