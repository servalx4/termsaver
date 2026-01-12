use super::Effect;
use crossterm::event::Event;
use std::io::{BufWriter, Stdout, Write};

// Ultra-fast noise implementation - much faster than Perlin
struct FastNoise {
    perm: [u8; 512],
}

impl FastNoise {
    fn new(seed: u32) -> Self {
        let mut perm = [0u8; 512];
        // Initialize permutation table with seed
        for i in 0..256 {
            perm[i] = i as u8;
        }

        // Fisher-Yates shuffle with seed
        let mut rng_state = seed;
        for i in (1..256).rev() {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng_state % (i as u32 + 1)) as usize;
            perm.swap(i, j);
        }

        // Duplicate for wraparound
        for i in 0..256 {
            perm[i + 256] = perm[i];
        }

        Self { perm }
    }

    #[inline]
    fn grad(&self, hash: u8, x: f32, y: f32) -> f32 {
        // Simple gradient selection - 8 directions
        let h = hash & 7;
        let u = if h < 4 { x } else { y };
        let v = if h < 4 { y } else { x };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    }

    #[inline]
    fn get(&self, x: f64, y: f64) -> f32 {
        // Fast floor
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;

        let xf = (x - xi as f64) as f32;
        let yf = (y - yi as f64) as f32;

        // Simpler fade curve - much faster than 5th order polynomial
        // Still smooth but 2x faster to compute
        let u = xf * xf * (3.0 - 2.0 * xf);
        let v = yf * yf * (3.0 - 2.0 * yf);

        // Hash coordinates
        let x0 = (xi & 255) as usize;
        let x1 = ((xi + 1) & 255) as usize;
        let y0 = (yi & 255) as usize;
        let y1 = ((yi + 1) & 255) as usize;

        let aa = self.perm[self.perm[x0] as usize + y0];
        let ab = self.perm[self.perm[x0] as usize + y1];
        let ba = self.perm[self.perm[x1] as usize + y0];
        let bb = self.perm[self.perm[x1] as usize + y1];

        // Gradients
        let g00 = self.grad(aa, xf, yf);
        let g10 = self.grad(ba, xf - 1.0, yf);
        let g01 = self.grad(ab, xf, yf - 1.0);
        let g11 = self.grad(bb, xf - 1.0, yf - 1.0);

        // Bilinear interpolation
        let x1_interp = g00 + u * (g10 - g00);
        let x2_interp = g01 + u * (g11 - g01);

        x1_interp + v * (x2_interp - x1_interp)
    }
}

#[derive(Clone)]
enum CloudType {
    Cumulus,        // Puffy white clouds
    Cirrus,         // Wispy high-altitude
    Stratus,        // Flat layers
    Cumulonimbus,   // Storm clouds
}

#[derive(Clone)]
struct CloudLayer {
    cloud_type: CloudType,
    altitude: f32,      // 0.0 to 1.0, affects position and parallax
    speed: f32,         // Drift speed
    density: f32,       // How thick the clouds are
    offset_x: f32,      // Current horizontal offset
    offset_y: f32,      // Vertical offset
    scale: f32,         // Noise scale
}

pub struct CloudEffect {
    width: usize,
    height: usize,
    time: f32,
    noise1: FastNoise,
    noise2: FastNoise,
    noise3: FastNoise,
    noise4: FastNoise,
    layers: Vec<CloudLayer>,
    output_buf: Vec<u8>,
}

impl CloudLayer {
    fn new(cloud_type: CloudType, altitude: f32) -> Self {
        let (speed, density, scale) = match cloud_type {
            CloudType::Cumulus => (
                0.5 + fastrand::f32() * 0.3,
                0.7 + fastrand::f32() * 0.25, // Higher density - more visible
                0.015,
            ),
            CloudType::Cirrus => (
                1.2 + fastrand::f32() * 0.5,
                0.15 + fastrand::f32() * 0.1, // Lower density - less overwhelming
                0.025,
            ),
            CloudType::Stratus => (
                0.3 + fastrand::f32() * 0.2,
                0.5 + fastrand::f32() * 0.2,
                0.008,
            ),
            CloudType::Cumulonimbus => (
                0.4 + fastrand::f32() * 0.2,
                0.85 + fastrand::f32() * 0.15, // Very high density - dramatic and visible
                0.012,
            ),
        };

        Self {
            cloud_type,
            altitude,
            speed,
            density,
            offset_x: fastrand::f32() * 1000.0,
            offset_y: fastrand::f32() * 1000.0,
            scale,
        }
    }
}

impl Effect for CloudEffect {
    fn new(width: usize, height: usize) -> Self {
        let mut layers = Vec::new();

        // Rebalanced for more variety - less cirrus, more cumulus and storms

        // Just one or two cirrus layers (not overwhelming)
        if fastrand::f32() > 0.3 {
            layers.push(CloudLayer::new(CloudType::Cirrus, 0.88));
        }

        // LOTS of cumulus clouds (main attraction)
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.72));
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.67));
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.62));
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.57));
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.53));
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.49));
        layers.push(CloudLayer::new(CloudType::Cumulus, 0.45));

        // Stratus for variety
        if fastrand::f32() > 0.5 {
            layers.push(CloudLayer::new(CloudType::Stratus, 0.40));
        }

        // Storm clouds are common and dramatic - towering vertically
        if fastrand::f32() > 0.2 {
            layers.push(CloudLayer::new(CloudType::Cumulonimbus, 0.60));
        }
        if fastrand::f32() > 0.4 {
            layers.push(CloudLayer::new(CloudType::Cumulonimbus, 0.54));
        }
        if fastrand::f32() > 0.6 {
            layers.push(CloudLayer::new(CloudType::Cumulonimbus, 0.48));
        }

        Self {
            width,
            height,
            time: 0.0,
            noise1: FastNoise::new(fastrand::u32(..)),
            noise2: FastNoise::new(fastrand::u32(..)),
            noise3: FastNoise::new(fastrand::u32(..)),
            noise4: FastNoise::new(fastrand::u32(..)),
            layers,
            output_buf: Vec::with_capacity(width * height * 25),
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;

        // Update layer positions - much slower drift
        for layer in &mut self.layers {
            layer.offset_x += layer.speed * dt * 0.8; // Reduced from 3.0 to 0.8
            // Wrap offset to prevent floating point precision issues with large numbers
            // Noise repeats every 256 units, so wrapping keeps performance consistent
            if layer.offset_x > 10000.0 {
                layer.offset_x -= 10000.0;
            }
        }
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H");

        let bg_color = crate::get_bg_color();

        // Sky gradient - light blue at top, lighter at horizon
        let sky_top = if bg_color == (0, 0, 0) {
            (135, 206, 235) // Sky blue
        } else {
            bg_color
        };
        let sky_horizon = if bg_color == (0, 0, 0) {
            (200, 230, 255) // Lighter blue at horizon
        } else {
            bg_color
        };

        let mut frame_buffer = vec![(0.0f32, 0.0f32, 0.0f32); self.width * self.height];

        // Initialize with sky gradient
        for y in 0..self.height {
            let t = y as f32 / self.height as f32;
            let r = sky_top.0 as f32 * (1.0 - t) + sky_horizon.0 as f32 * t;
            let g = sky_top.1 as f32 * (1.0 - t) + sky_horizon.1 as f32 * t;
            let b = sky_top.2 as f32 * (1.0 - t) + sky_horizon.2 as f32 * t;

            for x in 0..self.width {
                let idx = y * self.width + x;
                frame_buffer[idx] = (r, g, b);
            }
        }

        // Render cloud layers from back to front (high to low altitude)
        let mut sorted_layers = self.layers.clone();
        sorted_layers.sort_by(|a, b| b.altitude.partial_cmp(&a.altitude).unwrap());

        for layer in &sorted_layers {
            self.render_cloud_layer(layer, &mut frame_buffer);
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

impl CloudEffect {
    fn render_cloud_layer(&self, layer: &CloudLayer, buffer: &mut [(f32, f32, f32)]) {
        // Cumulonimbus clouds need different vertical treatment
        let is_vertical = matches!(layer.cloud_type, CloudType::Cumulonimbus);

        for y in 0..self.height {
            let distance = y as f32 / self.height as f32;
            let perspective_scale = 0.3 + distance * 2.0;
            let scale = layer.scale * perspective_scale;

            for x in 0..self.width {
                let idx = y * self.width + x;

                let nx = (x as f64 * scale as f64) + layer.offset_x as f64;
                // Cumulonimbus uses much more vertical sampling for towering effect
                let vertical_scale = if is_vertical { 1.8 } else { 0.3 };
                let ny = (y as f64 * scale as f64 * vertical_scale) + layer.offset_y as f64;

                let alt_offset = layer.altitude as f64 * 100.0;

                // Double domain warp for more organic shapes
                let warp1 = self.noise3.get(nx * 0.08, ny * 0.08 + alt_offset * 3.0);
                let warp2 = self.noise4.get(nx * 0.12, ny * 0.12 + alt_offset * 2.5);
                let warped_nx = nx + (warp1 * 10.0 + warp2 * 5.0) as f64;
                let warped_ny = ny + (warp1 * 10.0 + warp2 * 5.0) as f64;

                // 4 octaves of noise for great detail with better performance
                let n1 = self.noise1.get(warped_nx * 0.25, warped_ny * 0.25 + alt_offset);
                let n2 = self.noise2.get(warped_nx * 0.7, warped_ny * 0.7 + alt_offset);
                let n3 = self.noise3.get(warped_nx * 2.2, warped_ny * 2.2 + alt_offset);
                let n4 = self.noise4.get(warped_nx * 6.5, warped_ny * 6.5 + alt_offset);

                // Highly detailed cloud value using 4 octaves
                let cloud_value = match layer.cloud_type {
                    CloudType::Cumulus => {
                        // Puffy billowy cumulus with great detail
                        let fbm = n1 * 0.45 + n2 * 0.3 + n3 * 0.18 + n4 * 0.07;
                        if fbm > 0.28 { // Lower threshold - more visible
                            let shape = (fbm - 0.28) * 1.39;
                            // Add fine detail to edges
                            let edge_detail = 1.0 + n4 * 0.4;
                            if fbm > 0.45 {
                                shape * edge_detail
                            } else {
                                shape * 0.68 * edge_detail
                            }
                        } else {
                            0.0
                        }
                    }
                    CloudType::Cirrus => {
                        // Wispy with fibrous detail
                        let fbm = n1 * 0.35 + n2 * 0.32 + n3 * 0.22 + n4 * 0.11;
                        if fbm > 0.18 { // Higher threshold - less overwhelming
                            let v = (fbm - 0.18) * 1.22;
                            // Add streaky detail
                            let streak = 1.0 + n4 * 0.5;
                            v * v * 0.5 * streak
                        } else {
                            0.0
                        }
                    }
                    CloudType::Stratus => {
                        // Layered with texture detail
                        let fbm = n1 * 0.5 + n2 * 0.3 + n3 * 0.15 + n4 * 0.05;
                        if fbm > -0.08 {
                            let v = (fbm + 1.0) * 0.5;
                            // Add subtle texture variation
                            let texture = 1.0 + n4 * 0.2;
                            v * v * 0.7 * texture
                        } else {
                            0.0
                        }
                    }
                    CloudType::Cumulonimbus => {
                        // Towering storm clouds with dramatic detail and extreme vertical extent
                        let fbm = n1 * 0.5 + n2 * 0.28 + n3 * 0.15 + n4 * 0.07;

                        // Lower threshold for more visible towering clouds
                        if fbm > 0.22 {
                            let shape = (fbm - 0.22) * 1.28;
                            // Add turbulent detail to storm clouds
                            let turbulence = 1.0 + (n4 * n4) * 0.5;
                            // Strong vertical gradient - clouds tower upward dramatically
                            let vertical_pos = y as f32 / self.height as f32;
                            // Much stronger boost toward top of screen (distant/high clouds)
                            let height_boost = 1.0 + (1.0 - vertical_pos) * 0.6;
                            shape * turbulence * height_boost
                        } else {
                            0.0
                        }
                    }
                };

                let density = (cloud_value * layer.density).clamp(0.0, 1.0);

                if density > 0.08 {
                    // Multi-scale shading for realistic lighting
                    let large_shade = n2 * 0.14; // Large-scale shadows
                    let medium_shade = n3 * 0.1; // Medium detail
                    let fine_shade = n4 * 0.06; // Fine detail

                    let vertical_shade = 0.65 + large_shade + medium_shade + fine_shade;

                    let base_brightness = match layer.cloud_type {
                        CloudType::Cumulus => 260.0,
                        CloudType::Cirrus => 252.0,
                        CloudType::Stratus => 215.0,
                        CloudType::Cumulonimbus => 155.0, // Darker for dramatic storm clouds
                    };

                    let brightness = base_brightness * vertical_shade.clamp(0.45, 1.05);
                    let atmo_fade = distance * 0.22;

                    let sky_r = buffer[idx].0;
                    let sky_g = buffer[idx].1;
                    let sky_b = buffer[idx].2;

                    let cloud_r = brightness * (1.0 - atmo_fade) + sky_r * atmo_fade;
                    let cloud_g = brightness * (1.0 - atmo_fade) + sky_g * atmo_fade;
                    let cloud_b = brightness * (1.0 - atmo_fade) + sky_b * atmo_fade;

                    buffer[idx].0 = buffer[idx].0 * (1.0 - density) + cloud_r * density;
                    buffer[idx].1 = buffer[idx].1 * (1.0 - density) + cloud_g * density;
                    buffer[idx].2 = buffer[idx].2 * (1.0 - density) + cloud_b * density;
                }
            }
        }
    }
}
