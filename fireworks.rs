use super::Effect;
use std::io::{BufWriter, Stdout, Write};

// Realistic firework colors based on chemical compounds
const COLORS: [(u8, u8, u8); 10] = [
    (255, 30, 30),    // Strontium (intense red)
    (220, 50, 50),    // Lithium (medium red)
    (255, 140, 0),    // Calcium (orange)
    (255, 220, 0),    // Sodium (yellow)
    (0, 255, 100),    // Barium (green)
    (60, 120, 255),   // Copper halides (blue)
    (100, 100, 255),  // Caesium (indigo)
    (180, 50, 255),   // Potassium/Rubidium (violet)
    (255, 200, 50),   // Charcoal/Iron (gold)
    (255, 255, 255),  // Titanium/Magnesium (white)
];

// Bright golden color for sparks
const SPARK_COLOR: (u8, u8, u8) = (255, 240, 100);

#[derive(Clone, Copy)]
enum ExplosionType {
    Sphere,       // Normal sphere explosion
    Ring,         // Flat ring/circle
    Willow,       // Downward arcing
    Crossette,    // Secondary explosions
    Strobe,       // Blinking particles
    MultiBurst,   // Multiple sequential bursts
    ColorShift,   // Particles change color
    Spiral,       // Spiral pattern
    Heart,        // Heart shape
    Star,         // Star shape
    Chrysanthemum, // Long trails
    DoubleExplosion, // Concentric bursts
    Willowtail,   // Releases golden sparks while falling
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    color: (u8, u8, u8),
    color_end: Option<(u8, u8, u8)>, // For color shifting
    strobe_phase: f32, // For strobe effect
    crossette_time: Option<f32>, // Time until secondary explosion
    trail_length: usize, // For chrysanthemum trails
    emits_sparks: bool, // For willowtail effect
    spark_timer: f32, // Timer for spark emission
    opacity: f32, // Randomized opacity for sparks (0.6-1.0)
}

#[derive(Clone)]
struct Rocket {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    target_y: f32,
    color: (u8, u8, u8),
    explosion_type: ExplosionType,
    burst_count: usize, // For multi-burst
}

impl ExplosionType {
    fn random() -> Self {
        match fastrand::usize(0..13) {
            0 => ExplosionType::Sphere,
            1 => ExplosionType::Ring,
            2 => ExplosionType::Willow,
            3 => ExplosionType::Crossette,
            4 => ExplosionType::Strobe,
            5 => ExplosionType::MultiBurst,
            6 => ExplosionType::ColorShift,
            7 => ExplosionType::Spiral,
            8 => ExplosionType::Heart,
            9 => ExplosionType::Star,
            10 => ExplosionType::Chrysanthemum,
            11 => ExplosionType::DoubleExplosion,
            _ => ExplosionType::Willowtail,
        }
    }
}

pub struct FireworksEffect {
    width: usize,
    height: usize,
    rockets: Vec<Rocket>,
    particles: Vec<Particle>,
    time: f32,
    next_launch: f32,
    output_buf: Vec<u8>,
}

impl Effect for FireworksEffect {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rockets: Vec::new(),
            particles: Vec::new(),
            time: 0.0,
            next_launch: 0.5,
            output_buf: Vec::with_capacity(width * height * 25),
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues
        if self.time > 10000.0 {
            self.time -= 10000.0;
            self.next_launch -= 10000.0;
        }

        // Launch new rockets
        if self.time >= self.next_launch {
            let x = fastrand::usize(self.width / 4..self.width * 3 / 4) as f32;
            let target_y = fastrand::usize(self.height / 6..self.height * 2 / 5) as f32;
            let color = COLORS[fastrand::usize(0..COLORS.len())];

            // Random angle from -5 to 5 degrees
            let angle_degrees = -5.0 + fastrand::f32() * 10.0;
            let angle_radians = angle_degrees * std::f32::consts::PI / 180.0;

            // Randomize launch velocity - ensure minimum to reach decent height
            let speed = 60.0 + fastrand::f32() * 70.0; // 60 to 130
            let vx = speed * angle_radians.sin();
            let vy = -speed * angle_radians.cos(); // Negative because up is negative y

            let explosion_type = ExplosionType::random();
            let burst_count = if matches!(explosion_type, ExplosionType::MultiBurst) {
                2 + fastrand::usize(0..2) // 2 or 3 bursts
            } else {
                1
            };

            self.rockets.push(Rocket {
                x,
                y: self.height as f32,
                vx,
                vy,
                target_y,
                color,
                explosion_type,
                burst_count,
            });

            self.next_launch = self.time + 0.3 + fastrand::f32() * 0.8;
        }

        // Update rockets and collect explosions
        let mut rockets_to_explode = Vec::new();

        self.rockets.retain_mut(|rocket| {
            rocket.x += rocket.vx * dt;
            rocket.y += rocket.vy * dt;
            rocket.vy += 60.0 * dt; // Gravity

            // Explode when reaching target height or velocity becomes positive
            if rocket.y <= rocket.target_y || rocket.vy > 0.0 {
                rockets_to_explode.push(rocket.clone());
                false // Remove rocket
            } else {
                true // Keep rocket
            }
        });

        // Create explosions
        for rocket in rockets_to_explode {
            self.create_explosion(&rocket);
        }

        // Update particles
        let mut crossette_explosions = Vec::new();
        let mut sparks_to_emit = Vec::new();

        self.particles.retain_mut(|particle| {
            // Skip particles that haven't "started" yet (multi-burst delayed particles)
            if particle.life < 0.0 {
                particle.life += dt;
                return true; // Keep but don't update position yet
            }

            particle.x += particle.vx * dt;
            particle.y += particle.vy * dt;
            particle.vx *= 0.98; // Air resistance
            particle.vy += 40.0 * dt; // Gravity
            particle.life -= dt;

            // Update strobe phase
            if particle.strobe_phase > 0.0 {
                particle.strobe_phase += dt * 15.0; // Fast oscillation
            }

            // Emit sparks for willowtail particles
            if particle.emits_sparks {
                particle.spark_timer -= dt;
                if particle.spark_timer <= 0.0 {
                    // Emit 2-4 golden sparks
                    let spark_count = 2 + fastrand::usize(0..3);
                    sparks_to_emit.push((particle.x, particle.y, particle.vx, particle.vy, spark_count));
                    particle.spark_timer = 0.03 + fastrand::f32() * 0.03; // Emit every 0.03-0.06s
                }
            }

            // Check for crossette secondary explosion
            if let Some(ref mut crossette_time) = particle.crossette_time {
                *crossette_time -= dt;
                if *crossette_time <= 0.0 {
                    // Schedule secondary explosion at particle's current position
                    crossette_explosions.push((particle.x, particle.y, particle.vx, particle.vy, particle.color));
                    return false; // Remove this particle
                }
            }

            particle.life > 0.0
        });

        // Create crossette secondary explosions (as golden sparks)
        for (x, y, vx, vy, _color) in crossette_explosions {
            let count = 15 + fastrand::usize(0..10);
            for _ in 0..count {
                let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
                let speed = fastrand::f32() * 25.0;

                self.particles.push(Particle {
                    x,
                    y,
                    vx: angle.cos() * speed + vx * 0.3, // Inherit some velocity
                    vy: angle.sin() * speed + vy * 0.3,
                    life: 1.0,
                    max_life: 0.4 + fastrand::f32() * 0.2, // Short-lived sparks (0.4-0.6)
                    color: SPARK_COLOR, // Golden sparks
                    color_end: None,
                    strobe_phase: 0.0,
                    crossette_time: None,
                    trail_length: 0,
                    emits_sparks: false,
                    spark_timer: 0.0,
                    opacity: 0.6 + fastrand::f32() * 0.4, // Random opacity 0.6-1.0
                });
            }
        }

        // Create golden sparks from willowtail particles
        for (x, y, vx, vy, count) in sparks_to_emit {
            for _ in 0..count {
                let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
                let speed = 3.0 + fastrand::f32() * 8.0;

                self.particles.push(Particle {
                    x,
                    y,
                    vx: angle.cos() * speed + vx * 0.5, // Inherit half the velocity
                    vy: angle.sin() * speed + vy * 0.5,
                    life: 1.0,
                    max_life: 0.3 + fastrand::f32() * 0.3, // Short-lived sparks
                    color: SPARK_COLOR, // Bright golden color
                    color_end: None,
                    strobe_phase: 0.0,
                    crossette_time: None,
                    trail_length: 1, // Small trail
                    emits_sparks: false,
                    spark_timer: 0.0,
                    opacity: 0.6 + fastrand::f32() * 0.4, // Random opacity 0.6-1.0
                });
            }
        }
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H");

        let bg_color = crate::get_bg_color();
        let mut glow_buffer = vec![(0.0f32, bg_color); self.width * self.height];

        // Draw rockets (ascending)
        for rocket in &self.rockets {
            let x = rocket.x as usize;
            let y = rocket.y as usize;

            if x < self.width && y < self.height {
                let idx = y * self.width + x;
                glow_buffer[idx] = (3.0, rocket.color);

                // Trail - follows the rocket's trajectory
                let vel_magnitude = (rocket.vx * rocket.vx + rocket.vy * rocket.vy).sqrt();
                if vel_magnitude > 0.0 {
                    let trail_dx = -rocket.vx / vel_magnitude;
                    let trail_dy = -rocket.vy / vel_magnitude;

                    for i in 1..5 {
                        let trail_x = (rocket.x + trail_dx * i as f32) as isize;
                        let trail_y = (rocket.y + trail_dy * i as f32) as isize;
                        if trail_x >= 0 && trail_x < self.width as isize && trail_y >= 0 && trail_y < self.height as isize {
                            let idx = (trail_y as usize) * self.width + (trail_x as usize);
                            let fade = 1.0 - (i as f32 * 0.2);
                            if fade > glow_buffer[idx].0 {
                                glow_buffer[idx] = (fade * 2.0, rocket.color);
                            }
                        }
                    }
                }
            }
        }

        // Draw particles in two passes: sparks first (underneath), then shells (on top)
        // Pass 1: Draw sparks (short-lived golden particles)
        for particle in &self.particles {
            // Skip particles that haven't "started" yet
            if particle.life < 0.0 {
                continue;
            }

            // Only render sparks in this pass (max_life < 0.7)
            if particle.max_life >= 0.7 {
                continue;
            }

            let fade = particle.life / particle.max_life;

            // Calculate color (handle color shifting)
            let current_color = if let Some(end_color) = particle.color_end {
                let t = 1.0 - fade; // Progress from start to end
                (
                    (particle.color.0 as f32 * (1.0 - t) + end_color.0 as f32 * t) as u8,
                    (particle.color.1 as f32 * (1.0 - t) + end_color.1 as f32 * t) as u8,
                    (particle.color.2 as f32 * (1.0 - t) + end_color.2 as f32 * t) as u8,
                )
            } else {
                particle.color
            };

            // Handle strobe effect (blink on/off)
            let strobe_visible = if particle.strobe_phase > 0.0 {
                particle.strobe_phase.sin() > 0.0
            } else {
                true
            };

            if !strobe_visible {
                continue;
            }

            // Draw trails (chrysanthemum and willow effects)
            if particle.trail_length > 0 {
                let vel_magnitude = (particle.vx * particle.vx + particle.vy * particle.vy).sqrt();
                if vel_magnitude > 0.1 {
                    let trail_dx = -particle.vx / vel_magnitude * 0.3;
                    let trail_dy = -particle.vy / vel_magnitude * 0.3;

                    for i in 1..=particle.trail_length {
                        let trail_x = (particle.x + trail_dx * i as f32) as i32;
                        let trail_y = (particle.y + trail_dy * i as f32) as i32;

                        if trail_x >= 0 && trail_x < self.width as i32 && trail_y >= 0 && trail_y < self.height as i32 {
                            let idx = trail_y as usize * self.width + trail_x as usize;
                            let trail_fade = fade * (1.0 - i as f32 / (particle.trail_length as f32 + 1.0));
                            let trail_intensity = trail_fade * 2.0;

                            if trail_intensity > glow_buffer[idx].0 {
                                glow_buffer[idx] = (trail_intensity, current_color);
                            }
                        }
                    }
                }
            }

            // Draw main spark particle (just 1 pixel, no glow)
            let x = particle.x as i32;
            let y = particle.y as i32;

            if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
                let idx = y as usize * self.width + x as usize;
                let intensity = fade * particle.opacity; // Use randomized opacity

                if intensity > glow_buffer[idx].0 {
                    glow_buffer[idx] = (intensity, current_color);
                }
            }
        }

        // Pass 2: Draw shells (main firework particles on top)
        for particle in &self.particles {
            // Skip particles that haven't "started" yet
            if particle.life < 0.0 {
                continue;
            }

            // Only render shells in this pass (max_life >= 0.7)
            if particle.max_life < 0.7 {
                continue;
            }

            let fade = particle.life / particle.max_life;

            // Calculate color (handle color shifting)
            let current_color = if let Some(end_color) = particle.color_end {
                let t = 1.0 - fade; // Progress from start to end
                (
                    (particle.color.0 as f32 * (1.0 - t) + end_color.0 as f32 * t) as u8,
                    (particle.color.1 as f32 * (1.0 - t) + end_color.1 as f32 * t) as u8,
                    (particle.color.2 as f32 * (1.0 - t) + end_color.2 as f32 * t) as u8,
                )
            } else {
                particle.color
            };

            // Handle strobe effect (blink on/off)
            let strobe_visible = if particle.strobe_phase > 0.0 {
                particle.strobe_phase.sin() > 0.0
            } else {
                true
            };

            if !strobe_visible {
                continue;
            }

            // Draw trails (chrysanthemum and willow effects)
            if particle.trail_length > 0 {
                let vel_magnitude = (particle.vx * particle.vx + particle.vy * particle.vy).sqrt();
                if vel_magnitude > 0.1 {
                    let trail_dx = -particle.vx / vel_magnitude * 0.3;
                    let trail_dy = -particle.vy / vel_magnitude * 0.3;

                    for i in 1..=particle.trail_length {
                        let trail_x = (particle.x + trail_dx * i as f32) as i32;
                        let trail_y = (particle.y + trail_dy * i as f32) as i32;

                        if trail_x >= 0 && trail_x < self.width as i32 && trail_y >= 0 && trail_y < self.height as i32 {
                            let idx = trail_y as usize * self.width + trail_x as usize;
                            let trail_fade = fade * (1.0 - i as f32 / (particle.trail_length as f32 + 1.0));
                            let trail_intensity = trail_fade * 2.0;

                            if trail_intensity > glow_buffer[idx].0 {
                                glow_buffer[idx] = (trail_intensity, current_color);
                            }
                        }
                    }
                }
            }

            // Draw main particle
            let x = particle.x as i32;
            let y = particle.y as i32;

            if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
                let idx = y as usize * self.width + x as usize;
                let intensity = fade * 2.5;

                // Shells always overwrite sparks underneath
                glow_buffer[idx] = (intensity, current_color);

                // Small glow around particle (keep max for glow to blend nicely)
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.height as i32 {
                            let idx = ny as usize * self.width + nx as usize;
                            let glow = fade * 0.8;
                            if glow > glow_buffer[idx].0 {
                                glow_buffer[idx] = (glow, current_color);
                            }
                        }
                    }
                }
            }
        }

        let mut prev_top_color: (u8, u8, u8) = (255, 255, 255);
        let mut prev_bot_color: (u8, u8, u8) = (255, 255, 255);

        // Render using half-blocks
        for y in (0..self.height).step_by(2) {
            for x in 0..self.width {
                let top_idx = y * self.width + x;
                let bot_idx = if y + 1 < self.height {
                    (y + 1) * self.width + x
                } else {
                    top_idx
                };

                let (top_intensity, top_base_color) = glow_buffer[top_idx];
                let (bot_intensity, bot_base_color) = glow_buffer[bot_idx];

                // Blend particle color with background based on intensity
                let top_color = if top_intensity > 0.05 {
                    let blend = (top_intensity / 3.0).min(1.0); // Normalize intensity
                    (
                        (bg_color.0 as f32 * (1.0 - blend) + top_base_color.0 as f32 * blend) as u8,
                        (bg_color.1 as f32 * (1.0 - blend) + top_base_color.1 as f32 * blend) as u8,
                        (bg_color.2 as f32 * (1.0 - blend) + top_base_color.2 as f32 * blend) as u8,
                    )
                } else {
                    bg_color
                };

                let bot_color = if bot_intensity > 0.05 {
                    let blend = (bot_intensity / 3.0).min(1.0); // Normalize intensity
                    (
                        (bg_color.0 as f32 * (1.0 - blend) + bot_base_color.0 as f32 * blend) as u8,
                        (bg_color.1 as f32 * (1.0 - blend) + bot_base_color.1 as f32 * blend) as u8,
                        (bg_color.2 as f32 * (1.0 - blend) + bot_base_color.2 as f32 * blend) as u8,
                    )
                } else {
                    bg_color
                };

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
}

impl FireworksEffect {
    fn create_explosion(&mut self, rocket: &Rocket) {
        match rocket.explosion_type {
            ExplosionType::Sphere => self.create_sphere_explosion(rocket),
            ExplosionType::Ring => self.create_ring_explosion(rocket),
            ExplosionType::Willow => self.create_willow_explosion(rocket),
            ExplosionType::Crossette => self.create_crossette_explosion(rocket),
            ExplosionType::Strobe => self.create_strobe_explosion(rocket),
            ExplosionType::MultiBurst => self.create_multiburst_explosion(rocket),
            ExplosionType::ColorShift => self.create_colorshift_explosion(rocket),
            ExplosionType::Spiral => self.create_spiral_explosion(rocket),
            ExplosionType::Heart => self.create_heart_explosion(rocket),
            ExplosionType::Star => self.create_star_explosion(rocket),
            ExplosionType::Chrysanthemum => self.create_chrysanthemum_explosion(rocket),
            ExplosionType::DoubleExplosion => self.create_double_explosion(rocket),
            ExplosionType::Willowtail => self.create_willowtail_explosion(rocket),
        }
    }

    fn create_sphere_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 80 + fastrand::usize(0..40);
        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = fastrand::f32() * 55.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.0 + fastrand::f32() * 0.5,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_ring_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 60 + fastrand::usize(0..30);
        for i in 0..particle_count {
            let angle = (i as f32 / particle_count as f32) * std::f32::consts::PI * 2.0;
            let speed = 35.0 + fastrand::f32() * 15.0;

            // Ring is horizontal, so vx and vy form the ring, vz would be minimal
            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: fastrand::f32() * 5.0 - 2.5 + rocket.vy, // Minimal vertical spread
                life: 1.0,
                max_life: 1.2 + fastrand::f32() * 0.3,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_willow_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 100 + fastrand::usize(0..50);
        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = fastrand::f32() * 40.0;

            // Bias towards downward motion
            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed.abs() * 0.3; // Reduced upward, emphasize downward

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: vx + rocket.vx,
                vy: vy + rocket.vy,
                life: 1.0,
                max_life: 1.5 + fastrand::f32() * 0.8, // Longer life for willow
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 3, // Longer trails
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_crossette_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 40 + fastrand::usize(0..20);
        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = 20.0 + fastrand::f32() * 30.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.0 + fastrand::f32() * 0.5,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: Some(0.2 + fastrand::f32() * 0.3), // Explode again after 0.2-0.5s
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_strobe_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 80 + fastrand::usize(0..40);
        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = fastrand::f32() * 50.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.0 + fastrand::f32() * 0.5,
                color: rocket.color,
                color_end: None,
                strobe_phase: fastrand::f32() * 6.28, // Random starting phase
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_multiburst_explosion(&mut self, rocket: &Rocket) {
        // Create smaller first burst
        let particle_count = 30 + fastrand::usize(0..20);
        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = 15.0 + fastrand::f32() * 20.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 0.6,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }

        // Schedule additional bursts by creating delayed "mini rockets"
        for burst in 1..rocket.burst_count {
            let delay = burst as f32 * 0.15;
            // Create particles that will appear later (simulated by short max_life that increases)
            let particle_count = 40 + fastrand::usize(0..30);
            for _ in 0..particle_count {
                let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
                let speed = fastrand::f32() * 45.0;

                self.particles.push(Particle {
                    x: rocket.x,
                    y: rocket.y - rocket.vy * delay, // Offset position for delay effect
                    vx: angle.cos() * speed + rocket.vx,
                    vy: angle.sin() * speed + rocket.vy,
                    life: 0.0 - delay, // Negative life = delayed start
                    max_life: 1.0 + delay,
                    color: rocket.color,
                    color_end: None,
                    strobe_phase: 0.0,
                    crossette_time: None,
                    trail_length: 0,
                    emits_sparks: false,
                    spark_timer: 0.0,
                    opacity: 1.0,
                });
            }
        }
    }

    fn create_colorshift_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 80 + fastrand::usize(0..40);
        // Pick a second color different from the rocket color
        let end_color = loop {
            let c = COLORS[fastrand::usize(0..COLORS.len())];
            if c != rocket.color { break c; }
        };

        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = fastrand::f32() * 50.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.2 + fastrand::f32() * 0.5,
                color: rocket.color,
                color_end: Some(end_color),
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_spiral_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 60;
        let spiral_turns = 3.0;

        for i in 0..particle_count {
            let t = i as f32 / particle_count as f32;
            let angle = t * std::f32::consts::PI * 2.0 * spiral_turns;
            let speed = 25.0 + t * 20.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.0 + fastrand::f32() * 0.5,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_heart_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 80;

        for i in 0..particle_count {
            let t = (i as f32 / particle_count as f32) * std::f32::consts::PI * 2.0;
            // Heart shape parametric equations
            let x_shape = 16.0 * t.sin().powi(3);
            let y_shape = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());

            let scale = 2.5;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: x_shape * scale + rocket.vx,
                vy: y_shape * scale + rocket.vy,
                life: 1.0,
                max_life: 1.3 + fastrand::f32() * 0.4,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_star_explosion(&mut self, rocket: &Rocket) {
        let points = 5;
        let particles_per_point = 15;

        for p in 0..points {
            let base_angle = (p as f32 / points as f32) * std::f32::consts::PI * 2.0;

            for i in 0..particles_per_point {
                let t = i as f32 / particles_per_point as f32;
                let radius = 20.0 + t * 35.0;
                let angle_offset = if i % 2 == 0 { 0.3 } else { -0.3 };
                let angle = base_angle + angle_offset * (1.0 - t);

                self.particles.push(Particle {
                    x: rocket.x,
                    y: rocket.y,
                    vx: angle.cos() * radius + rocket.vx,
                    vy: angle.sin() * radius + rocket.vy,
                    life: 1.0,
                    max_life: 1.2 + fastrand::f32() * 0.3,
                    color: rocket.color,
                    color_end: None,
                    strobe_phase: 0.0,
                    crossette_time: None,
                    trail_length: 0,
                    emits_sparks: false,
                    spark_timer: 0.0,
                    opacity: 1.0,
                });
            }
        }
    }

    fn create_chrysanthemum_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 120 + fastrand::usize(0..60);

        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = fastrand::f32() * 45.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.8 + fastrand::f32() * 0.7, // Very long life
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 6, // Very long trails
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_double_explosion(&mut self, rocket: &Rocket) {
        // Inner fast burst
        let inner_count = 40 + fastrand::usize(0..20);
        for _ in 0..inner_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = 40.0 + fastrand::f32() * 25.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 0.8 + fastrand::f32() * 0.3,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }

        // Outer slow burst
        let outer_count = 60 + fastrand::usize(0..30);
        for _ in 0..outer_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = 10.0 + fastrand::f32() * 20.0;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: angle.cos() * speed + rocket.vx,
                vy: angle.sin() * speed + rocket.vy,
                life: 1.0,
                max_life: 1.3 + fastrand::f32() * 0.5,
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 0,
                emits_sparks: false,
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }

    fn create_willowtail_explosion(&mut self, rocket: &Rocket) {
        let particle_count = 40 + fastrand::usize(0..20);

        for _ in 0..particle_count {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;
            let speed = 10.0 + fastrand::f32() * 25.0;

            // Spread in a circle
            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed;

            self.particles.push(Particle {
                x: rocket.x,
                y: rocket.y,
                vx: vx + rocket.vx,
                vy: vy + rocket.vy,
                life: 1.0,
                max_life: 2.0 + fastrand::f32() * 1.0, // Long life for willowtail
                color: rocket.color,
                color_end: None,
                strobe_phase: 0.0,
                crossette_time: None,
                trail_length: 2,
                emits_sparks: true, // Willowtail emits golden sparks!
                spark_timer: 0.0,
                opacity: 1.0,
            });
        }
    }
}
