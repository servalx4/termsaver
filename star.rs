use super::Effect;
use crossterm::event::{Event, MouseEvent, MouseEventKind};
use std::io::{BufWriter, Stdout, Write};

// Custom fast noise - reusing from clouds
struct FastNoise {
    perm: [u8; 512],
}

impl FastNoise {
    fn new(seed: u32) -> Self {
        let mut perm = [0u8; 512];
        for i in 0..256 {
            perm[i] = i as u8;
        }

        let mut rng_state = seed;
        for i in (1..256).rev() {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng_state % (i as u32 + 1)) as usize;
            perm.swap(i, j);
        }

        for i in 0..256 {
            perm[i + 256] = perm[i];
        }

        Self { perm }
    }

    #[inline]
    fn grad(&self, hash: u8, x: f32, y: f32) -> f32 {
        let h = hash & 7;
        let u = if h < 4 { x } else { y };
        let v = if h < 4 { y } else { x };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    }

    #[inline]
    fn get(&self, x: f64, y: f64) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;

        let xf = (x - xi as f64) as f32;
        let yf = (y - yi as f64) as f32;

        let u = xf * xf * (3.0 - 2.0 * xf);
        let v = yf * yf * (3.0 - 2.0 * yf);

        let x0 = (xi & 255) as usize;
        let x1 = ((xi + 1) & 255) as usize;
        let y0 = (yi & 255) as usize;
        let y1 = ((yi + 1) & 255) as usize;

        let aa = self.perm[self.perm[x0] as usize + y0];
        let ab = self.perm[self.perm[x0] as usize + y1];
        let ba = self.perm[self.perm[x1] as usize + y0];
        let bb = self.perm[self.perm[x1] as usize + y1];

        let g00 = self.grad(aa, xf, yf);
        let g10 = self.grad(ba, xf - 1.0, yf);
        let g01 = self.grad(ab, xf, yf - 1.0);
        let g11 = self.grad(bb, xf - 1.0, yf - 1.0);

        let x1_interp = g00 + u * (g10 - g00);
        let x2_interp = g01 + u * (g11 - g01);

        x1_interp + v * (x2_interp - x1_interp)
    }
}

#[derive(Clone, Copy)]
enum LuminosityClass {
    MainSequence,     // V - normal stars
    Giant,            // III - evolved, large
    Supergiant,       // I - massive, enormous
}

struct Star {
    temperature: f32,  // Kelvin
    luminosity_class: LuminosityClass,
}

impl Star {
    fn random() -> Self {
        // Weight toward main sequence stars (most common in universe)
        let luminosity_class = match fastrand::u8(0..100) {
            0..=85 => LuminosityClass::MainSequence,
            86..=95 => LuminosityClass::Giant,
            _ => LuminosityClass::Supergiant,
        };

        // Generate temperature based on class
        let temperature = match luminosity_class {
            LuminosityClass::MainSequence => {
                // Main sequence: 3000K (red dwarfs) to 30000K (blue stars)
                // Weight toward cooler stars (more common)
                let roll = fastrand::f32();
                if roll < 0.6 {
                    // Red/orange dwarfs (most common)
                    3000.0 + fastrand::f32() * 2500.0  // 3000-5500K
                } else if roll < 0.85 {
                    // Yellow/white stars
                    5500.0 + fastrand::f32() * 3000.0  // 5500-8500K
                } else {
                    // Hot blue stars (rare)
                    8500.0 + fastrand::f32() * 21500.0  // 8500-30000K
                }
            }
            LuminosityClass::Giant => {
                // Giants are usually cooler (red/orange giants)
                3500.0 + fastrand::f32() * 2500.0  // 3500-6000K
            }
            LuminosityClass::Supergiant => {
                // Supergiants can be red OR blue
                if fastrand::bool() {
                    // Red supergiant
                    3200.0 + fastrand::f32() * 1500.0  // 3200-4700K
                } else {
                    // Blue supergiant
                    10000.0 + fastrand::f32() * 20000.0  // 10000-30000K
                }
            }
        };

        Self {
            temperature,
            luminosity_class,
        }
    }

    fn properties(&self) -> StarProperties {
        // Calculate color from temperature (blackbody radiation)
        let color = Self::temperature_to_color(self.temperature);

        // Use temperature as deterministic seed for variation (wrapping to prevent overflow)
        let temp_seed = (self.temperature * 1000.0) as u32;
        let variation1 = ((temp_seed.wrapping_mul(2654435761)) % 1000) as f32 / 1000.0;
        let variation2 = ((temp_seed.wrapping_mul(2654435789)) % 1000) as f32 / 1000.0;
        let variation3 = ((temp_seed.wrapping_mul(2654435823)) % 1000) as f32 / 1000.0;

        // Calculate radius based on luminosity class and temperature
        let radius_scale = match self.luminosity_class {
            LuminosityClass::MainSequence => {
                // Hotter main sequence stars are bigger
                // 3000K -> 0.2, 6000K -> 0.45, 30000K -> 0.8
                let temp_factor = ((self.temperature - 3000.0) / 27000.0).clamp(0.0, 1.0);
                0.2 + temp_factor * 0.6
            }
            LuminosityClass::Giant => {
                // Giants are much bigger than main sequence
                0.65 + variation1 * 0.2  // 0.65-0.85
            }
            LuminosityClass::Supergiant => {
                // Supergiants are enormous
                0.8 + variation1 * 0.15  // 0.8-0.95
            }
        };

        // Cooler stars have more convection -> more surface activity
        // Hotter stars are more radiative -> less surface activity
        let activity_level = if self.temperature < 7000.0 {
            // Convective stars (red, orange, yellow)
            0.6 + (7000.0 - self.temperature) / 7000.0 * 0.6  // 0.6-1.2
        } else {
            // Radiative stars (white, blue)
            0.35 + variation2 * 0.25  // 0.35-0.6
        };

        // Granulation scale based on convection zone depth
        let granulation_scale = if self.temperature < 6000.0 {
            0.15 + (6000.0 - self.temperature) / 3000.0 * 0.15  // Cooler = larger cells
        } else {
            0.05 + variation3 * 0.05
        };

        // Flare frequency based on real stellar physics
        // Red dwarfs (M-type, <4000K) are EXTREMELY flare-prone
        // Orange/yellow stars moderate activity
        // Flare chances further reduced
        let flare_chance = if self.temperature < 4000.0 {
            // Red dwarfs - flare stars! Superflares common
            0.25 + variation2 * 0.15  // Frequent: 0.25-0.4 per second
        } else if self.temperature < 5000.0 {
            // Orange dwarfs - active but not as extreme
            0.12 + variation2 * 0.08  // Moderate: 0.12-0.2
        } else if self.temperature < 6500.0 {
            // Yellow stars (like Sun) - relatively calm
            0.05 + variation2 * 0.05  // Calm: 0.05-0.1
        } else if self.temperature < 8000.0 {
            // White stars - quiet
            0.02 + variation2 * 0.02  // Rare: 0.02-0.04
        } else {
            // Hot blue stars - very few flares
            0.01 + variation2 * 0.01  // Very rare: 0.01-0.02
        };

        StarProperties {
            radius_scale,
            color,
            activity_level,
            granulation_scale,
            flare_chance,
        }
    }

    fn temperature_to_color(temp: f32) -> (u8, u8, u8) {
        // Simplified blackbody color approximation
        let temp_clamped = temp.clamp(1000.0, 40000.0);

        let r = if temp_clamped < 6600.0 {
            255.0
        } else {
            let t = (temp_clamped - 6000.0) / 100.0;
            (329.698727446 * (t - 60.0).powf(-0.1332047592)).clamp(0.0, 255.0)
        };

        let g = if temp_clamped < 6600.0 {
            let t = temp_clamped / 100.0;
            (99.4708025861 * t.ln() - 161.1195681661).clamp(0.0, 255.0)
        } else {
            let t = (temp_clamped - 6000.0) / 100.0;
            (288.1221695283 * (t - 60.0).powf(-0.0755148492)).clamp(0.0, 255.0)
        };

        let b = if temp_clamped >= 6600.0 {
            255.0
        } else if temp_clamped <= 2000.0 {
            0.0
        } else {
            let t = (temp_clamped - 1000.0) / 100.0;
            (138.5177312231 * (t - 10.0).ln() - 305.0447927307).clamp(0.0, 255.0)
        };

        (r as u8, g as u8, b as u8)
    }
}

struct StarProperties {
    radius_scale: f32,
    color: (u8, u8, u8),
    activity_level: f32,
    granulation_scale: f32,
    flare_chance: f32,
}

struct Flare {
    angle: f32,
    height: f32,        // Current arc height
    max_height: f32,    // Maximum arc height
    arc_width: f32,     // Width of the horseshoe base (expands over time)
    base_arc_width: f32, // Initial arc width
    thickness: f32,     // Thickness of the prominence
    radial_offset: f32, // How far the base is pushed out from star surface
    intensity: f32,
    base_intensity: f32, // Base intensity (for red dwarf superflares)
    lifetime: f32,
    max_lifetime: f32,
    noise_offset: f32,  // For unique turbulence per flare
}

pub struct StarEffect {
    width: usize,
    height: usize,
    time: f32,
    _star: Star,
    props: StarProperties,
    star_name: String,
    mass: f32,          // Solar masses
    radius: f32,        // Solar radii
    luminosity: f32,    // Solar luminosities
    rotation: f32,
    noise1: FastNoise,
    noise2: FastNoise,
    noise3: FastNoise,
    flares: Vec<Flare>,
    last_click_time: f32,  // Cooldown to prevent double-clicks
    output_buf: Vec<u8>,
}

impl Effect for StarEffect {
    fn new(width: usize, height: usize) -> Self {
        let star = Star::random();
        let props = star.properties();

        // Generate star name
        let star_name = Self::generate_star_name();

        // Calculate physical properties based on temperature and luminosity class
        let (mass, radius, luminosity) = Self::calculate_star_stats(&star);

        Self {
            width,
            height,
            time: 0.0,
            _star: star,
            props,
            star_name,
            mass,
            radius,
            luminosity,
            rotation: 0.0,
            noise1: FastNoise::new(fastrand::u32(..)),
            noise2: FastNoise::new(fastrand::u32(..)),
            noise3: FastNoise::new(fastrand::u32(..)),
            flares: Vec::new(),
            last_click_time: 0.0,
            output_buf: Vec::with_capacity(width * height * 25),
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        if self.time > 10000.0 {
            self.time -= 10000.0;
        }

        // Slow rotation
        self.rotation += dt * 0.05;
        if self.rotation > std::f32::consts::PI * 2.0 {
            self.rotation -= std::f32::consts::PI * 2.0;
        }

        let props = &self.props;

        // Spawn flares (solar prominences)
        if fastrand::f32() < props.flare_chance * dt {
            let angle = fastrand::f32() * std::f32::consts::PI * 2.0;

            // Red dwarf superflares are more dramatic
            let is_red_dwarf = self._star.temperature < 4000.0;
            let is_supergiant = matches!(self._star.luminosity_class, LuminosityClass::Supergiant);

            let (max_height, max_width, intensity_range, lifetime_range) = if is_red_dwarf {
                // Red dwarf superflares - MASSIVE and bright, longer lived
                (0.6, 0.5, (1.2, 2.0), (15.0, 25.0))
            } else if is_supergiant {
                // Supergiant prominences - huge but less intense, very long lived
                (0.8, 0.6, (0.8, 1.2), (20.0, 35.0))
            } else {
                // Normal prominences - longer lived
                (0.5, 0.35, (0.7, 1.0), (10.0, 18.0))
            };

            // Add significant randomization to each flare
            let size_variation = 0.6 + fastrand::f32() * 0.8; // 0.6x to 1.4x size
            let brightness_variation = 0.7 + fastrand::f32() * 0.6; // 0.7x to 1.3x brightness
            let lifetime_variation = 0.7 + fastrand::f32() * 0.6; // 0.7x to 1.3x lifetime

            let randomized_max_height = max_height * size_variation;
            let base_intensity = (intensity_range.0 + fastrand::f32() * (intensity_range.1 - intensity_range.0)) * brightness_variation;
            let randomized_lifetime = (lifetime_range.0 + fastrand::f32() * (lifetime_range.1 - lifetime_range.0)) * lifetime_variation;

            let base_arc_width = (0.15 + fastrand::f32() * max_width) * size_variation;
            self.flares.push(Flare {
                angle,
                height: 0.0,
                max_height: randomized_max_height,
                arc_width: base_arc_width,
                base_arc_width,
                thickness: (0.08 + fastrand::f32() * 0.08) * size_variation,
                radial_offset: 0.0,
                intensity: 0.0,
                base_intensity,
                lifetime: 0.0,
                max_lifetime: randomized_lifetime,
                noise_offset: fastrand::f32() * 1000.0,
            });
        }

        // Update flares
        self.flares.retain_mut(|flare| {
            flare.lifetime += dt;
            let life_ratio = flare.lifetime / flare.max_lifetime;

            // Flare erupts and continuously moves away from star, expanding in all directions
            flare.radial_offset = life_ratio * 6.0; // Faster outward movement

            if life_ratio < 0.08 {
                // Initial eruption - height grows while moving away
                let growth = life_ratio / 0.08;
                flare.height = growth * flare.max_height;
                flare.intensity = growth * flare.base_intensity;
                // Start expanding in all directions from the beginning
                let initial_thickness = 0.12;
                flare.thickness = initial_thickness * (1.0 + life_ratio * 5.0);
                // Horizontal spreading
                flare.arc_width = flare.base_arc_width * (1.0 + life_ratio * 3.0);
            } else {
                // Continuous expansion and dissipation
                flare.height = flare.max_height;
                // Cloud disperses - expands rapidly in all directions
                let initial_thickness = 0.12;
                flare.thickness = initial_thickness * (1.0 + life_ratio * 5.0);
                // Horizontal spreading continues
                flare.arc_width = flare.base_arc_width * (1.0 + life_ratio * 3.0);
                // Fade intensity as it moves away
                let dissipate = (life_ratio - 0.08) / 0.92;
                flare.intensity = (1.0 - dissipate).powf(3.5) * flare.base_intensity;
            }

            flare.lifetime < flare.max_lifetime
        });
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H");

        let bg_color = crate::get_bg_color();
        let space_color = if bg_color == (0, 0, 0) {
            (5, 5, 15)
        } else {
            bg_color
        };

        let props = &self.props;

        // Calculate star center and radius
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;
        let base_radius = (self.width.min(self.height) as f32 * 0.4) * props.radius_scale;

        let mut frame_buffer = vec![(space_color.0 as f32, space_color.1 as f32, space_color.2 as f32); self.width * self.height];

        // Render star
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < base_radius * 1.4 {
                    let idx = y * self.width + x;

                    // Calculate angle and distance from center
                    let angle = dy.atan2(dx) + self.rotation;
                    let normalized_dist = dist / base_radius;

                    // Surface coordinate with rotation
                    let surface_x = angle * base_radius * 0.5;
                    let surface_y = normalized_dist * base_radius;

                    // Multi-octave granulation (convection cells) - faster movement
                    let gran1 = self.noise1.get(
                        surface_x as f64 * props.granulation_scale as f64,
                        surface_y as f64 * props.granulation_scale as f64 + self.time as f64 * 0.3,
                    );
                    let gran2 = self.noise2.get(
                        surface_x as f64 * props.granulation_scale as f64 * 3.0,
                        surface_y as f64 * props.granulation_scale as f64 * 3.0 + self.time as f64 * 0.45,
                    );
                    let gran3 = self.noise3.get(
                        surface_x as f64 * props.granulation_scale as f64 * 8.0,
                        surface_y as f64 * props.granulation_scale as f64 * 8.0 + self.time as f64 * 0.6,
                    );

                    // Much more visible and varied granulation
                    let granulation = gran1 * 0.6 + gran2 * 0.4 + gran3 * 0.3;

                    // Limb darkening - star is darker at edges but not too dark
                    let limb_factor = if normalized_dist < 1.0 {
                        let center_brightness = (1.0 - normalized_dist.powf(1.2)).clamp(0.0, 1.0);
                        let limb_darkening = 0.65 + center_brightness * 0.35;  // 65%-100% brightness
                        limb_darkening
                    } else {
                        0.0
                    };

                    if normalized_dist < 1.0 {
                        // Core star surface with strong granulation variation - much brighter base
                        let surface_brightness = 1.5 + granulation * 0.7 * props.activity_level;
                        let brightness = surface_brightness * limb_factor;

                        let mut r = props.color.0 as f32 * brightness;
                        let mut g = props.color.1 as f32 * brightness;
                        let mut b = props.color.2 as f32 * brightness;

                        // Add edge gradient to blend into corona (avoids hard edge)
                        if normalized_dist > 0.92 {
                            let edge_blend = ((normalized_dist - 0.92) / 0.08).clamp(0.0, 1.0);
                            // Sample corona at this angle
                            let corona_sample_dist = 1.05; // Just into corona
                            let corona_noise = self.noise1.get(
                                angle as f64 * 25.0,
                                corona_sample_dist as f64 * 2.0 + self.time as f64 * 0.4,
                            ) * 0.5 + 0.5;
                            let corona_intensity = 0.6 * corona_noise;

                            // Blend surface with corona color
                            r += props.color.0 as f32 * corona_intensity * edge_blend;
                            g += props.color.1 as f32 * corona_intensity * edge_blend;
                            b += props.color.2 as f32 * corona_intensity * edge_blend;
                        }

                        frame_buffer[idx].0 = r.min(255.0);
                        frame_buffer[idx].1 = g.min(255.0);
                        frame_buffer[idx].2 = b.min(255.0);
                    } else if normalized_dist < 1.4 {
                        // Variable corona extent based on angle
                        let extent_noise = self.noise3.get(
                            angle as f64 * 12.0,
                            self.time as f64 * 0.2,
                        ) * 0.5 + 0.5;

                        // Some rays extend to 1.4, some only to 1.15
                        let max_extent = 1.15 + extent_noise * 0.25;

                        if normalized_dist < max_extent {
                            // Corona/chromosphere with irregular detail
                            let corona_dist = (normalized_dist - 1.0) / (max_extent - 1.0);

                            // Streaky corona noise - high frequency angular, low frequency radial
                            // Creates ray-like streaks emanating from star surface
                            let corona_noise1 = self.noise1.get(
                                angle as f64 * 25.0,  // High frequency around star
                                normalized_dist as f64 * 2.0 + self.time as f64 * 0.4,  // Low frequency - long streaks
                            ) * 0.5 + 0.5;
                            let corona_noise2 = self.noise2.get(
                                angle as f64 * 40.0,  // Even higher frequency for detail
                                normalized_dist as f64 * 3.0 + self.time as f64 * 0.6,
                            ) * 0.5 + 0.5;

                            // Combine noises for detail - much more variation
                            let corona_detail = corona_noise1 * 0.7 + corona_noise2 * 0.3;

                            // Base falloff with strong detail modulation - brighter
                            let base_intensity = (1.0 - corona_dist).powf(1.5) * 1.1;
                            // Much stronger noise influence - can go from 0.3 to 1.8x (brighter range)
                            let corona_intensity = base_intensity * (0.3 + corona_detail * 1.5);

                            frame_buffer[idx].0 += props.color.0 as f32 * corona_intensity;
                            frame_buffer[idx].1 += props.color.1 as f32 * corona_intensity;
                            frame_buffer[idx].2 += props.color.2 as f32 * corona_intensity;
                        }
                    }
                }
            }
        }

        // Add twinkling stars in background
        self.add_stars(&mut frame_buffer);

        // Render flares/prominences
        for flare in &self.flares {
            self.render_flare(&mut frame_buffer, center_x, center_y, base_radius, flare, &props);
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

        // Add star info overlay in top left
        self.output_buf.extend_from_slice(b"\x1b[1;2H"); // Position at row 1, col 2
        self.output_buf.extend_from_slice(b"\x1b[38;2;255;255;255m"); // White text
        self.output_buf.extend_from_slice(b"\x1b[1m"); // Bold
        write!(self.output_buf, "{}", self.star_name)?;

        self.output_buf.extend_from_slice(b"\x1b[2;2H"); // Row 2
        self.output_buf.extend_from_slice(b"\x1b[0m\x1b[38;2;200;200;200m"); // Dimmer white
        write!(self.output_buf, "Mass: {:.2} M☉", self.mass)?;

        self.output_buf.extend_from_slice(b"\x1b[3;2H"); // Row 3
        write!(self.output_buf, "Radius: {:.1} R☉", self.radius)?;

        self.output_buf.extend_from_slice(b"\x1b[4;2H"); // Row 4
        write!(self.output_buf, "Luminosity: {:.1} L☉", self.luminosity)?;

        self.output_buf.extend_from_slice(b"\x1b[5;2H"); // Row 5
        write!(self.output_buf, "Temp: {:.0} K", self._star.temperature)?;

        self.output_buf.extend_from_slice(b"\x1b[0m"); // Reset formatting

        stdout.write_all(&self.output_buf)?;
        stdout.flush()?;
        Ok(())
    }

    fn handle_event(&mut self, event: &Event) {
        if let Event::Mouse(MouseEvent { kind, column, row, .. }) = event {
            if matches!(kind, MouseEventKind::Down(_)) {
                // Cooldown to prevent double-clicks (200ms)
                if self.time - self.last_click_time < 0.2 {
                    return;
                }

                // Limit total flares for performance
                if self.flares.len() >= 8 {
                    return;
                }

                self.last_click_time = self.time;

                // Calculate star center
                let center_x = self.width as f32 / 2.0;
                let center_y = self.height as f32 / 2.0;

                // Mouse click position (accounting for half-block rendering)
                let target_x = *column as f32;
                let target_y = *row as f32 * 2.0; // Double because of half-blocks

                // Calculate angle toward click
                let dx = target_x - center_x;
                let dy = target_y - center_y;
                let angle = dy.atan2(dx);

                // Spawn single dramatic prominence flare toward cursor
                let size_variation = 1.2 + fastrand::f32() * 0.3; // 1.2x to 1.5x
                let base_arc_width = (0.25 + fastrand::f32() * 0.15) * size_variation;

                self.flares.push(Flare {
                    angle,
                    height: 0.0,
                    max_height: 0.8 * size_variation, // Very tall and dramatic
                    arc_width: base_arc_width,
                    base_arc_width,
                    thickness: (0.12 + fastrand::f32() * 0.05) * size_variation,
                    radial_offset: 0.0,
                    intensity: 0.0,
                    base_intensity: 1.8 + fastrand::f32() * 0.4, // Very bright
                    lifetime: 0.0,
                    max_lifetime: 3.0 + fastrand::f32() * 1.5, // 3-4.5 seconds for better performance
                    noise_offset: fastrand::f32() * 1000.0,
                });
            }
        }
    }
}

impl StarEffect {
    fn render_flare(&self, buffer: &mut [(f32, f32, f32)], center_x: f32, center_y: f32, radius: f32, flare: &Flare, props: &StarProperties) {
        // Create horseshoe-shaped prominence that arcs up and back down
        // The prominence loops from one side of the base to the other

        let base_angle_left = flare.angle - flare.arc_width / 2.0;
        let base_angle_right = flare.angle + flare.arc_width / 2.0;

        // Base stays anchored to star surface
        let left_base_x = center_x + base_angle_left.cos() * radius;
        let left_base_y = center_y + base_angle_left.sin() * radius;
        let right_base_x = center_x + base_angle_right.cos() * radius;
        let right_base_y = center_y + base_angle_right.sin() * radius;

        // Sample many points along the horseshoe arc
        let num_segments = 40;  // Reduced for better performance

        for i in 0..=num_segments {
            let t = i as f32 / num_segments as f32;

            // Parabolic arc: goes up in the middle, down at the ends
            // Arc extends outward as flare ages (radial_offset pushes it out)
            let arc_height = (1.0 - (t * 2.0 - 1.0).powi(2)) * radius * (flare.height + flare.radial_offset);

            // Interpolate base position
            let base_x = left_base_x + (right_base_x - left_base_x) * t;
            let base_y = left_base_y + (right_base_y - left_base_y) * t;

            // Direction away from star center
            let to_center_x = center_x - base_x;
            let to_center_y = center_y - base_y;
            let to_center_len = (to_center_x * to_center_x + to_center_y * to_center_y).sqrt();
            let away_x = -to_center_x / to_center_len;
            let away_y = -to_center_y / to_center_len;

            // Add noise-based turbulence - subtle
            let turb_x = self.noise1.get(
                (t * 10.0 + flare.noise_offset) as f64,
                (self.time * 0.8 + flare.noise_offset) as f64,
            ) * radius * 0.12;
            let turb_y = self.noise2.get(
                (t * 10.0 + flare.noise_offset + 100.0) as f64,
                (self.time * 0.8 + flare.noise_offset + 100.0) as f64,
            ) * radius * 0.12;

            // Final position with arc height and turbulence
            let x = base_x + away_x * arc_height + turb_x;
            let y = base_y + away_y * arc_height + turb_y;

            // Intensity falls off at the ends of the arc
            let edge_falloff = (t * (1.0 - t) * 4.0).min(1.0);

            // Add strong detail variation along the prominence
            let detail_noise = self.noise3.get(
                (t * 20.0 + flare.noise_offset) as f64,
                (self.time * 0.5) as f64,
            ) * 0.5 + 0.5;

            // Much stronger modulation - can vary from 0.3 to 1.3x
            let intensity = flare.intensity * edge_falloff * (0.3 + detail_noise * 1.0);

            // Thickness varies along arc - thicker at the tip (middle), thinner at base (ends)
            // Parabolic profile: thickest at t=0.5
            let thickness_profile = 1.0 - (t * 2.0 - 1.0).powi(2); // 0 at ends, 1 at middle
            let variable_thickness = flare.thickness * (0.5 + thickness_profile * 1.5); // 0.5x to 2x base thickness

            // Draw thick volumetric prominence
            let thickness_pixels = (radius * variable_thickness) as i32;
            for dx in -thickness_pixels..=thickness_pixels {
                for dy in -thickness_pixels..=thickness_pixels {
                    let px = (x + dx as f32) as i32;
                    let py = (y + dy as f32) as i32;

                    // CRITICAL: Skip any pixels outside buffer bounds completely
                    // This prevents ANY processing or buffer access for out-of-bounds pixels
                    if px < 0 || py < 0 {
                        continue;  // Off top or left edge
                    }
                    if px >= self.width as i32 || py >= self.height as i32 {
                        continue;  // Off right or bottom edge
                    }

                    let dist = ((dx * dx + dy * dy) as f32).sqrt() / thickness_pixels as f32;
                    if dist < 1.0 {
                        // Reduce intensity near screen edges to prevent visible stacking
                        let edge_distance = px.min(py).min(self.width as i32 - px - 1).min(self.height as i32 - py - 1);
                        let edge_fade = if edge_distance < 5 {
                            edge_distance as f32 / 5.0
                        } else {
                            1.0
                        };
                            // Create visible filaments running along the flare's actual curved direction
                            // Calculate the tangent to the arc at this point
                            // Tangent has two components:
                            // 1. Horizontal motion along base (left to right)
                            let base_tangent_x = right_base_x - left_base_x;
                            let base_tangent_y = right_base_y - left_base_y;

                            // 2. Vertical motion from parabolic arc height
                            // Derivative of (1 - (2t-1)^2) = -4(2t-1)
                            let arc_derivative = -4.0 * (2.0 * t - 1.0) * radius * (flare.height + flare.radial_offset);

                            let tangent_x = base_tangent_x + away_x * arc_derivative;
                            let tangent_y = base_tangent_y + away_y * arc_derivative;
                            let tangent_len = (tangent_x * tangent_x + tangent_y * tangent_y).sqrt();
                            let tangent_norm_x = tangent_x / tangent_len;
                            let tangent_norm_y = tangent_y / tangent_len;

                            // Project pixel offset perpendicular to tangent for filament coordinate
                            let perpendicular_dist = -(dx as f32 * tangent_norm_y - dy as f32 * tangent_norm_x);

                            // Lower frequency - wider, more visible streaks
                            let filament_coord = (perpendicular_dist * 1.5 + flare.noise_offset * 10.0) as f64;

                            // Primary filament structure
                            let filament1 = self.noise1.get(
                                (t * 4.0 + flare.noise_offset) as f64,
                                filament_coord * 0.4,
                            );

                            // Secondary layer for variation
                            let filament2 = self.noise2.get(
                                (t * 7.0 + flare.noise_offset + 200.0) as f64,
                                filament_coord * 0.7,
                            );

                            // Combine
                            let combined = filament1 * 0.6 + filament2 * 0.4;

                            // Sharp threshold for visible bands
                            let texture = if combined > 0.1 {
                                1.0  // Fully bright
                            } else if combined > -0.3 {
                                0.4  // Medium
                            } else {
                                0.1  // Dark gaps
                            };

                            // Apply texture
                            let base_glow = (1.0 - dist).powf(1.5);
                            let glow = base_glow * intensity * texture * edge_fade;
                            let idx = py as usize * self.width + px as usize;

                            // Prominences glow slightly brighter
                            let brightness_boost = 1.1;
                            buffer[idx].0 = (buffer[idx].0 + props.color.0 as f32 * glow * brightness_boost).min(255.0);
                            buffer[idx].1 = (buffer[idx].1 + props.color.1 as f32 * glow * brightness_boost).min(255.0);
                            buffer[idx].2 = (buffer[idx].2 + props.color.2 as f32 * glow * brightness_boost).min(255.0);
                    }
                }
            }
        }
    }

    fn generate_star_name() -> String {
        // Generate procedural random names from syllables
        const CONSONANTS: &[&str] = &[
            "b", "c", "d", "f", "g", "h", "j", "k", "l", "m",
            "n", "p", "r", "s", "t", "v", "w", "x", "z",
            "th", "ch", "sh", "ph", "kr", "tr", "dr", "br", "gr"
        ];

        const VOWELS: &[&str] = &[
            "a", "e", "i", "o", "u", "ae", "ei", "ou", "ia", "eo"
        ];

        const ENDINGS: &[&str] = &[
            "or", "an", "en", "on", "ar", "is", "us", "os", "as", "ax",
            "ix", "ex", "yx", "ion", "ius", "ara", "iel", "ath", "oth", "eth"
        ];

        let mut name = String::new();

        // Bias towards shorter names: 2 syllables most common, sometimes 3
        let num_syllables = if fastrand::f32() < 0.7 {
            2
        } else {
            3
        };

        for i in 0..num_syllables {
            // Start with consonant (or sometimes vowel for variety)
            if i == 0 || fastrand::f32() > 0.3 {
                name.push_str(CONSONANTS[fastrand::usize(..CONSONANTS.len())]);
            }
            name.push_str(VOWELS[fastrand::usize(..VOWELS.len())]);

            // Sometimes add consonant after vowel (not at end)
            if i < num_syllables - 1 && fastrand::f32() > 0.6 {
                name.push_str(CONSONANTS[fastrand::usize(..CONSONANTS.len())]);
            }
        }

        // Add ending suffix less often for shorter names
        if fastrand::f32() > 0.7 {
            name.push_str(ENDINGS[fastrand::usize(..ENDINGS.len())]);
        }

        // Capitalize first letter
        let mut chars: Vec<char> = name.chars().collect();
        if let Some(first) = chars.get_mut(0) {
            *first = first.to_uppercase().next().unwrap_or(*first);
        }

        chars.into_iter().collect()
    }

    fn calculate_star_stats(star: &Star) -> (f32, f32, f32) {
        // Calculate mass, radius, and luminosity based on temperature and class
        let (mass, radius, luminosity) = match star.luminosity_class {
            LuminosityClass::MainSequence => {
                // Main sequence: mass-luminosity relation
                // M ~ (T/5778)^4 for main sequence
                let temp_ratio = star.temperature / 5778.0; // Relative to Sun
                let mass = temp_ratio.powf(2.5).clamp(0.08, 20.0);
                let radius = temp_ratio.powf(0.8).clamp(0.1, 15.0);
                let luminosity = temp_ratio.powf(4.0).clamp(0.0001, 10000.0);
                (mass, radius, luminosity)
            }
            LuminosityClass::Giant => {
                // Giants are evolved stars - larger radius, moderate mass
                let temp_ratio = star.temperature / 4500.0;
                let mass = (0.8 + fastrand::f32() * 2.0).clamp(0.8, 8.0);
                let radius = (10.0 + fastrand::f32() * 40.0).clamp(10.0, 100.0);
                let luminosity = (radius * radius * temp_ratio.powf(4.0)).clamp(10.0, 1000.0);
                (mass, radius, luminosity)
            }
            LuminosityClass::Supergiant => {
                // Supergiants - massive and extremely luminous
                let temp_ratio = star.temperature / 4000.0;
                let mass = (10.0 + fastrand::f32() * 30.0).clamp(10.0, 50.0);
                let radius = (100.0 + fastrand::f32() * 400.0).clamp(100.0, 1000.0);
                let luminosity = (radius * radius * temp_ratio.powf(4.0)).clamp(1000.0, 100000.0);
                (mass, radius, luminosity)
            }
        };

        (mass, radius, luminosity)
    }

    fn add_stars(&self, buffer: &mut [(f32, f32, f32)]) {
        // Add subtle twinkling stars in the background
        let star_density = 0.004; // Slightly more than aurora for space theme
        let num_stars = (self.width * self.height) as f32 * star_density;

        for i in 0..num_stars as usize {
            // Use deterministic seed for consistent star positions
            let star_seed = i as f64 * 123.456;
            let x = ((star_seed * 7919.0) % self.width as f64) as usize;
            let y = ((star_seed * 7907.0) % self.height as f64) as usize;

            // Random twinkle parameters per star
            let phase = (star_seed % 1000.0) as f32 / 1000.0 * std::f32::consts::PI * 2.0;
            let twinkle_speed = 0.3 + ((star_seed % 200.0) / 100.0) as f32;

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
                let brightness = (twinkle - 0.3) * 100.0;
                let idx = y * self.width + x;

                // White stars with slight color variation
                buffer[idx].0 = (buffer[idx].0 + brightness * 0.95).min(255.0);
                buffer[idx].1 = (buffer[idx].1 + brightness * 0.97).min(255.0);
                buffer[idx].2 = (buffer[idx].2 + brightness).min(255.0);
            }
        }
    }
}
