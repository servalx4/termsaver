use super::Effect;
use crossterm::event::Event;
use noise::{NoiseFn, Perlin};
use std::io::{BufWriter, Stdout, Write};

const AURORA_COLORS: [(u8, u8, u8); 5] = [
    (30, 255, 120),   // Bright green
    (50, 200, 220),   // Cyan
    (100, 150, 255),  // Light blue
    (180, 100, 255),  // Purple
    (255, 80, 180),   // Magenta/pink
];

pub struct AuroraEffect {
    width: usize,
    height: usize,
    time: f32,
    noise: Perlin,
    output_buf: Vec<u8>,
    curtains: Vec<AuroraCurtain>,
}

struct AuroraCurtain {
    base_y: f32,
    color_idx: usize,
    wave_offset: f32,
    wave_speed: f32,
    wave_amplitude: f32,
    intensity: f32,
    height_scale: f32,
}

impl AuroraCurtain {
    fn new(color_idx: usize, base_y_ratio: f32) -> Self {
        Self {
            base_y: base_y_ratio,
            color_idx,
            wave_offset: fastrand::f32() * 100.0,
            wave_speed: 0.3 + fastrand::f32() * 0.4,
            wave_amplitude: 8.0 + fastrand::f32() * 12.0,
            intensity: 0.35 + fastrand::f32() * 0.25,  // Reduced from 0.6-1.0 to 0.35-0.6
            height_scale: 0.3 + fastrand::f32() * 0.5,
        }
    }
}

impl Effect for AuroraEffect {
    fn new(width: usize, height: usize) -> Self {
        // Create multiple aurora curtains at different heights
        let curtains = vec![
            AuroraCurtain::new(0, 0.2),  // Green curtain
            AuroraCurtain::new(1, 0.3),  // Cyan curtain
            AuroraCurtain::new(2, 0.25), // Blue curtain
            AuroraCurtain::new(3, 0.35), // Purple curtain
            AuroraCurtain::new(4, 0.28), // Magenta curtain
        ];

        Self {
            width,
            height,
            time: 0.0,
            noise: Perlin::new(fastrand::u32(..)),
            output_buf: Vec::with_capacity(width * height * 25),
            curtains,
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues
        if self.time > 10000.0 {
            self.time -= 10000.0;
        }
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H");

        let bg_color = crate::get_bg_color();
        // Initialize with background color to avoid artifacts
        let bg_float = (bg_color.0 as f32, bg_color.1 as f32, bg_color.2 as f32);
        let mut frame_buffer = vec![bg_float; self.width * self.height];

        // Render each curtain
        for curtain in &self.curtains {
            let base_color = AURORA_COLORS[curtain.color_idx];
            let curtain_base_y = curtain.base_y * self.height as f32;

            for x in 0..self.width {
                // Use Perlin noise for smooth horizontal wave
                // Add curtain index to noise coordinates to avoid banding between curtains
                let noise_x = x as f64 * 0.015;  // Slightly coarser for smoother waves
                let noise_t = (self.time * curtain.wave_speed + curtain.wave_offset) as f64;
                let noise_z = curtain.color_idx as f64 * 10.0;  // Separate each curtain in noise space

                let wave_y = self.noise.get([noise_x, noise_t, noise_z]) as f32;
                let wave_offset = wave_y * curtain.wave_amplitude;

                // Second noise layer for vertical undulation (makes curtains taller/shorter)
                let vertical_noise = self.noise.get([noise_x * 0.5, noise_t * 0.7, noise_z + 100.0]) as f32;
                let height_variation = 1.0 + vertical_noise * 0.4;  // Reduced variation

                // Third noise layer for intensity variation
                let intensity_noise = self.noise.get([noise_x * 0.3, noise_t * 0.5, noise_z + 200.0]) as f32;
                let intensity_mod = 0.75 + (intensity_noise * 0.5 + 0.5) * 0.25;

                // Skip this entire column if intensity is too low
                let max_intensity = curtain.intensity * intensity_mod;
                if max_intensity < 0.25 {
                    continue;
                }

                let center_y = curtain_base_y + wave_offset;
                let curtain_height = self.height as f32 * curtain.height_scale * height_variation;

                // Draw vertical streaks with smooth falloff
                for dy in -(curtain_height as i32)..=(curtain_height as i32) {
                    let y = center_y as i32 + dy;

                    // Skip if outside screen bounds (don't clamp - that causes stacking)
                    if y < 0 || y >= self.height as i32 {
                        continue;
                    }

                    // Calculate distance from center for falloff
                    let dist = dy.abs() as f32 / curtain_height;
                    if dist > 1.0 {
                        continue;
                    }

                    // Smooth falloff using cosine curve
                    let falloff = ((1.0 - dist) * std::f32::consts::PI / 2.0).cos();
                    let falloff = falloff * falloff; // Square for sharper edges

                    let intensity = curtain.intensity * intensity_mod * falloff;

                    // Use a clear threshold to avoid very faint contributions
                    if intensity > 0.2 {
                        let idx = y as usize * self.width + x;

                        // Calculate actual color contribution
                        let r_add = base_color.0 as f32 * intensity;
                        let g_add = base_color.1 as f32 * intensity;
                        let b_add = base_color.2 as f32 * intensity;

                        // Only add if at least one channel adds a visible amount (>= 2 units)
                        if r_add >= 2.0 || g_add >= 2.0 || b_add >= 2.0 {
                            // Prevent overflow by checking before adding
                            // Also avoid adding to already-saturated pixels
                            if frame_buffer[idx].0 < 250.0 {
                                frame_buffer[idx].0 = (frame_buffer[idx].0 + r_add).min(255.0);
                            }
                            if frame_buffer[idx].1 < 250.0 {
                                frame_buffer[idx].1 = (frame_buffer[idx].1 + g_add).min(255.0);
                            }
                            if frame_buffer[idx].2 < 250.0 {
                                frame_buffer[idx].2 = (frame_buffer[idx].2 + b_add).min(255.0);
                            }
                        }
                    }
                }
            }
        }

        // Add stars twinkling in the background
        self.add_stars(&mut frame_buffer);

        // Convert frame buffer to colors and render
        let mut prev_top_color: (u8, u8, u8) = (255, 255, 255);
        let mut prev_bot_color: (u8, u8, u8) = (255, 255, 255);

        for y in (0..self.height).step_by(2) {
            for x in 0..self.width {
                let top_idx = y * self.width + x;
                let bot_idx = if y + 1 < self.height {
                    (y + 1) * self.width + x
                } else {
                    top_idx
                };

                let top_color = self.blend_with_background(frame_buffer[top_idx], bg_color);
                let bot_color = self.blend_with_background(frame_buffer[bot_idx], bg_color);

                if top_color != prev_top_color {
                    write!(
                        self.output_buf,
                        "\x1b[48;2;{};{};{}m",
                        top_color.0, top_color.1, top_color.2
                    )?;
                    prev_top_color = top_color;
                }
                if bot_color != prev_bot_color {
                    write!(
                        self.output_buf,
                        "\x1b[38;2;{};{};{}m",
                        bot_color.0, bot_color.1, bot_color.2
                    )?;
                    prev_bot_color = bot_color;
                }

                self.output_buf.extend_from_slice("▄".as_bytes());
            }
            self.output_buf.extend_from_slice(b"\x1b[0m");
            prev_top_color = (255, 255, 255);
            prev_bot_color = (255, 255, 255);
            if y + 2 < self.height {
                self.output_buf.extend_from_slice(b"\r\n");
            }
        }

        stdout.write_all(&self.output_buf)?;
        stdout.flush()?;
        Ok(())
    }

    fn handle_event(&mut self, _event: &Event) {}
}

impl AuroraEffect {
    fn add_stars(&self, buffer: &mut [(f32, f32, f32)]) {
        // Add subtle twinkling stars with random intervals
        let star_density = 0.003;
        let num_stars = (self.width * self.height) as f32 * star_density;

        for i in 0..num_stars as usize {
            // Use deterministic seed for consistent star positions
            let star_seed = i as f64 * 123.456;
            let x = ((star_seed * 7919.0) % self.width as f64) as usize;
            let y = ((star_seed * 7907.0) % self.height as f64) as usize;

            // Stars only in upper 2/3 of screen
            if y > (self.height * 2) / 3 {
                continue;
            }

            // Random twinkle parameters per star
            let phase = (star_seed % 1000.0) as f32 / 1000.0 * std::f32::consts::PI * 2.0;
            let twinkle_speed = 0.5 + ((star_seed % 200.0) / 100.0) as f32;

            // Some stars twinkle smoothly, others pulse on/off
            let twinkle_type = (star_seed % 3.0) as u32;
            let twinkle = match twinkle_type {
                0 => {
                    // Smooth sine wave twinkle
                    ((self.time * twinkle_speed + phase).sin() * 0.5 + 0.5).powf(2.0)
                }
                1 => {
                    // Sharp on/off pulses
                    let cycle = (self.time * twinkle_speed * 0.3 + phase).sin();
                    if cycle > 0.7 { 1.0 } else { 0.0 }
                }
                _ => {
                    // Random flickers - uses multiple frequencies
                    let fast = (self.time * twinkle_speed * 3.0 + phase).sin() * 0.5 + 0.5;
                    let slow = (self.time * twinkle_speed * 0.5 + phase * 2.0).sin() * 0.5 + 0.5;
                    (fast * slow).powf(2.5)
                }
            };

            if twinkle > 0.3 {
                let brightness = (twinkle - 0.3) * 120.0;
                let idx = y * self.width + x;

                // Slight blue-white tint to stars
                buffer[idx].0 = (buffer[idx].0 + brightness * 0.92).min(255.0);
                buffer[idx].1 = (buffer[idx].1 + brightness * 0.96).min(255.0);
                buffer[idx].2 = (buffer[idx].2 + brightness).min(255.0);
            }
        }
    }

    fn blend_with_background(&self, color: (f32, f32, f32), _bg: (u8, u8, u8)) -> (u8, u8, u8) {
        // Clamp color values (background is already included in frame buffer)
        (
            color.0.round().clamp(0.0, 255.0) as u8,
            color.1.round().clamp(0.0, 255.0) as u8,
            color.2.round().clamp(0.0, 255.0) as u8,
        )
    }
}
