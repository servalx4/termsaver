use super::Effect;
use crossterm::event::Event;
use std::io::{BufWriter, Stdout, Write};

const DEEP_WATER: (u8, u8, u8) = (2, 8, 20);
const FISH_GLOW: (u8, u8, u8) = (40, 150, 255);

struct Fish {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    target_vx: f32,
    target_vy: f32,
    size: f32,
    base_brightness: f32,
    brightness: f32,
    target_brightness: f32,
    blink_phase: f32,
    blink_speed: f32,
    blink_type: u8, // 0=smooth pulse, 1=sharp blink, 2=random flicker
    independence: f32, // How much this fish ignores the school (0.0 = follows, 1.0 = independent)
    trail: Vec<(f32, f32, f32)>, // (x, y, intensity)
}

impl Fish {
    fn new(x: f32, y: f32, size: f32) -> Self {
        let base_brightness = 0.5 + fastrand::f32() * 0.5;
        let blink_type = fastrand::u8(0..3);
        let independence = if fastrand::f32() < 0.3 {
            // 30% of fish are more independent
            0.5 + fastrand::f32() * 0.5
        } else {
            fastrand::f32() * 0.3
        };

        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            target_vx: 0.0,
            target_vy: 0.0,
            size,
            base_brightness,
            brightness: base_brightness,
            target_brightness: base_brightness,
            blink_phase: fastrand::f32() * std::f32::consts::PI * 2.0,
            blink_speed: 0.5 + fastrand::f32() * 2.5,
            blink_type,
            independence,
            trail: Vec::new(),
        }
    }

    fn update(&mut self, dt: f32, others: &[Fish], width: usize, height: usize, current_x: f32, current_y: f32, _time: f32) {
        // Update blink phase
        self.blink_phase += dt * self.blink_speed;
        if self.blink_phase > std::f32::consts::PI * 2.0 {
            self.blink_phase -= std::f32::consts::PI * 2.0;
        }

        // Calculate target brightness based on blink type
        self.target_brightness = match self.blink_type {
            0 => {
                // Smooth pulsing
                let pulse = (self.blink_phase.sin() * 0.5 + 0.5).powf(1.5);
                self.base_brightness * (0.3 + pulse * 0.7)
            }
            1 => {
                // Sharp blinks - on/off (but will fade smoothly to target)
                let cycle = self.blink_phase.sin();
                if cycle > 0.6 {
                    self.base_brightness
                } else {
                    self.base_brightness * 0.1
                }
            }
            _ => {
                // Random flickers
                let fast = (self.blink_phase * 2.3).sin() * 0.5 + 0.5;
                let slow = (self.blink_phase * 0.7).sin() * 0.5 + 0.5;
                let flicker = (fast * slow).powf(1.8);
                self.base_brightness * (0.2 + flicker * 0.8)
            }
        };

        // Smoothly interpolate brightness towards target
        let fade_speed = 8.0; // Higher = faster fade
        self.brightness += (self.target_brightness - self.brightness) * fade_speed * dt;
        self.brightness = self.brightness.clamp(0.0, 1.0);

        // Schooling behavior (boids) - reduced for more chaos
        let mut separation = (0.0f32, 0.0f32);
        let mut alignment = (0.0f32, 0.0f32);
        let mut cohesion = (0.0f32, 0.0f32);
        let mut nearby_count = 0;

        for other in others {
            let dx = other.x - self.x;
            let dy = other.y - self.y;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq > 0.1 && dist_sq < 900.0 {
                nearby_count += 1;

                // Separation - avoid crowding
                if dist_sq < 100.0 {
                    let dist = dist_sq.sqrt();
                    separation.0 -= dx / dist;
                    separation.1 -= dy / dist;
                }

                // Alignment - match velocity
                alignment.0 += other.vx;
                alignment.1 += other.vy;

                // Cohesion - move toward center
                cohesion.0 += dx;
                cohesion.1 += dy;
            }
        }

        if nearby_count > 0 {
            alignment.0 /= nearby_count as f32;
            alignment.1 /= nearby_count as f32;
            cohesion.0 /= nearby_count as f32;
            cohesion.1 /= nearby_count as f32;
        }

        // More frequent random darting for independent fish
        let dart_chance = 0.02 + self.independence * 0.06;
        if fastrand::f32() < dart_chance {
            self.target_vx = (fastrand::f32() - 0.5) * 80.0;
            self.target_vy = (fastrand::f32() - 0.5) * 60.0;
        }

        // Apply independence - reduce schooling forces
        let school_factor = 1.0 - self.independence;

        // Combine behaviors - reduced alignment and cohesion for more variety
        self.target_vx += separation.0 * 8.0 + alignment.0 * 0.2 * school_factor + cohesion.0 * 0.1 * school_factor;
        self.target_vy += separation.1 * 8.0 + alignment.1 * 0.2 * school_factor + cohesion.1 * 0.1 * school_factor;

        // Add current
        self.target_vx += current_x * 10.0;
        self.target_vy += current_y * 5.0;

        // Smooth acceleration
        self.vx += (self.target_vx - self.vx) * 0.05;
        self.vy += (self.target_vy - self.vy) * 0.05;

        // Speed limit
        let speed = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if speed > 50.0 {
            self.vx *= 50.0 / speed;
            self.vy *= 50.0 / speed;
        }

        // Drag
        self.vx *= 0.99;
        self.vy *= 0.99;

        // Update position
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // Wrap around
        if self.x < 0.0 { self.x += width as f32; }
        if self.x >= width as f32 { self.x -= width as f32; }
        if self.y < 0.0 { self.y += height as f32; }
        if self.y >= height as f32 { self.y -= height as f32; }

        // Update trail
        self.trail.push((self.x, self.y, 1.0));
        if self.trail.len() > 8 {
            self.trail.remove(0);
        }

        // Fade trail
        for point in &mut self.trail {
            point.2 *= 0.85;
        }
    }
}

pub struct BioluminescenceEffect {
    width: usize,
    height: usize,
    time: f32,
    fish: Vec<Fish>,
    output_buf: Vec<u8>,
}

impl Effect for BioluminescenceEffect {
    fn new(width: usize, height: usize) -> Self {
        let mut fish = Vec::new();

        // Spawn large swarm of fish
        for _ in 0..70 {
            let x = fastrand::f32() * width as f32;
            let y = fastrand::f32() * height as f32;
            let size = 1.0 + fastrand::f32() * 1.5;
            fish.push(Fish::new(x, y, size));
        }

        Self {
            width,
            height,
            time: 0.0,
            fish,
            output_buf: Vec::with_capacity(width * height * 25),
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues
        if self.time > 10000.0 {
            self.time -= 10000.0;
        }

        // Ambient current
        let current_x = (self.time * 0.3).sin() * 0.5 + (self.time * 0.7).cos() * 0.3;
        let current_y = (self.time * 0.4).sin() * 0.2;

        // Update fish with schooling
        let fish_clone: Vec<Fish> = self.fish.iter().map(|f| Fish {
            x: f.x,
            y: f.y,
            vx: f.vx,
            vy: f.vy,
            target_vx: f.target_vx,
            target_vy: f.target_vy,
            size: f.size,
            base_brightness: f.base_brightness,
            brightness: f.brightness,
            target_brightness: f.target_brightness,
            blink_phase: f.blink_phase,
            blink_speed: f.blink_speed,
            blink_type: f.blink_type,
            independence: f.independence,
            trail: Vec::new(),
        }).collect();

        for i in 0..self.fish.len() {
            self.fish[i].update(dt, &fish_clone, self.width, self.height, current_x, current_y, self.time);
        }
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H");

        let bg_color = crate::get_bg_color();
        let water_color = if bg_color == (0, 0, 0) {
            DEEP_WATER
        } else {
            bg_color
        };

        // Initialize frame buffer with deep water
        let mut frame_buffer = vec![(water_color.0 as f32, water_color.1 as f32, water_color.2 as f32); self.width * self.height];

        // Render fish trails
        for fish in &self.fish {
            for (tx, ty, intensity) in &fish.trail {
                if *intensity > 0.1 {
                    self.add_glow(&mut frame_buffer, *tx, *ty, 2.0, *intensity * fish.brightness, FISH_GLOW);
                }
            }
        }

        // Render fish
        for fish in &self.fish {
            self.add_glow(&mut frame_buffer, fish.x, fish.y, 2.5, fish.brightness, FISH_GLOW);
        }

        // Convert to output
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

                let top_color = (
                    frame_buffer[top_idx].0.round().clamp(0.0, 255.0) as u8,
                    frame_buffer[top_idx].1.round().clamp(0.0, 255.0) as u8,
                    frame_buffer[top_idx].2.round().clamp(0.0, 255.0) as u8,
                );
                let bot_color = (
                    frame_buffer[bot_idx].0.round().clamp(0.0, 255.0) as u8,
                    frame_buffer[bot_idx].1.round().clamp(0.0, 255.0) as u8,
                    frame_buffer[bot_idx].2.round().clamp(0.0, 255.0) as u8,
                );

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

impl BioluminescenceEffect {
    fn add_glow(&self, buffer: &mut [(f32, f32, f32)], x: f32, y: f32, radius: f32, intensity: f32, color: (u8, u8, u8)) {
        let x_min = (x - radius).max(0.0) as usize;
        let x_max = (x + radius).min(self.width as f32 - 1.0) as usize;
        let y_min = (y - radius).max(0.0) as usize;
        let y_max = (y + radius).min(self.height as f32 - 1.0) as usize;

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let dx = px as f32 - x;
                let dy = py as f32 - y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < radius {
                    let falloff = (1.0 - (dist / radius)).powf(2.0);
                    let contribution = intensity * falloff;

                    if contribution > 0.02 {
                        let idx = py * self.width + px;
                        buffer[idx].0 = (buffer[idx].0 + color.0 as f32 * contribution).min(255.0);
                        buffer[idx].1 = (buffer[idx].1 + color.1 as f32 * contribution).min(255.0);
                        buffer[idx].2 = (buffer[idx].2 + color.2 as f32 * contribution).min(255.0);
                    }
                }
            }
        }
    }
}
