use super::Effect;
use crossterm::event::{Event, MouseEvent, MouseEventKind};
use std::io::{BufWriter, Stdout, Write};

// Pink core with white glow
const CORE_COLORS: [(u8, u8, u8); 6] = [
    (255, 255, 255), // White glow
    (255, 200, 220), // Light pink glow
    (255, 120, 160), // Pink
    (255, 80, 140),  // Deep pink core
    (255, 60, 130),  // Deeper pink
    (230, 50, 120),  // Core center
];

#[derive(Clone)]
struct TendrilSegment {
    x: f32,
    y: f32,
    end_x: f32,
    end_y: f32,
    intensity: f32,
    initial_y: f32,      // Store initial Y for rise calculation
    initial_end_y: f32,  // Store initial end Y for rise calculation
}

struct Tendril {
    segments: Vec<TendrilSegment>,
    age: f32,
    max_age: f32,
    edge_segment: u8, // Which edge segment this tendril is targeting
    is_mouse_tendril: bool, // True if this tendril tracks mouse cursor
    target_x: f32, // Target position for mouse tendrils
    target_y: f32,
}

impl Tendril {
    fn new(
        center_x: f32,
        center_y: f32,
        orb_radius: f32,
        mouse_x: Option<f32>,
        mouse_y: Option<f32>,
        width: usize,
        height: usize,
        edge_segment: u8,
        is_mouse_tendril: bool,
    ) -> Self {
        // Determine target based on mouse or edge segment
        let (target_x, target_y) = if let (Some(mx), Some(my)) = (mouse_x, mouse_y) {
            (mx, my)
        } else {
            // Target based on edge segment (0-11, dividing perimeter into 12 parts)
            // Top edge: segments 0-2, Right edge: segments 3-5, Bottom edge: segments 6-8, Left edge: segments 9-11
            let perimeter = 2.0 * (width + height) as f32;
            let segment_length = perimeter / 12.0;
            let segment_start = edge_segment as f32 * segment_length;
            let offset = fastrand::f32() * segment_length; // Random position within segment

            let distance = segment_start + offset;

            // Walk around perimeter to find target point
            if distance < width as f32 {
                // Top edge
                (distance, 0.0)
            } else if distance < width as f32 + height as f32 {
                // Right edge
                ((width - 1) as f32, distance - width as f32)
            } else if distance < 2.0 * width as f32 + height as f32 {
                // Bottom edge
                (width as f32 - (distance - width as f32 - height as f32), (height - 1) as f32)
            } else {
                // Left edge
                (0.0, height as f32 - (distance - 2.0 * width as f32 - height as f32))
            }
        };

        // Calculate angle from center to target
        let dx = target_x - center_x;
        let dy = target_y - center_y;
        let base_angle = dy.atan2(dx);

        // Start from orb surface in direction of target
        let start_x = center_x + base_angle.cos() * orb_radius;
        let start_y = center_y + base_angle.sin() * orb_radius;

        let mut segments = Vec::new();
        Self::generate_tendril(
            &mut segments,
            start_x,
            start_y,
            target_x,
            target_y,
            1.0,
            0,
            width,
            height,
        );

        Self {
            segments,
            age: 0.0,
            max_age: if is_mouse_tendril {
                f32::INFINITY // Mouse tendrils never age out
            } else {
                0.3 + fastrand::f32() * 0.2 // Shorter lifetime: 0.3-0.5s
            },
            edge_segment,
            is_mouse_tendril,
            target_x,
            target_y,
        }
    }

    fn generate_tendril(
        segments: &mut Vec<TendrilSegment>,
        x: f32,
        y: f32,
        target_x: f32,
        target_y: f32,
        intensity: f32,
        generation: u8,
        width: usize,
        height: usize,
    ) {
        if generation > 2 || intensity < 0.3 {
            return;
        }

        if x < 0.0 || x >= width as f32 || y < 0.0 || y >= height as f32 {
            return;
        }

        let dx = target_x - x;
        let dy = target_y - y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 5.0 {
            if target_x >= 0.0 && target_x < width as f32 && target_y >= 0.0 && target_y < height as f32 {
                segments.push(TendrilSegment {
                    x,
                    y,
                    end_x: target_x,
                    end_y: target_y,
                    intensity,
                    initial_y: y,
                    initial_end_y: target_y,
                });
            }
            return;
        }

        // Create smooth path with Bezier-like curves
        let num_segments = (dist / 6.0).max(3.0) as usize;
        let mut current_x = x;
        let mut current_y = y;

        // Add control points for smooth random curves
        let mid_t = 0.5;
        let control_offset_x = (fastrand::f32() - 0.5) * dist * 0.3;
        let control_offset_y = (fastrand::f32() - 0.5) * dist * 0.3;
        let control_x = x + dx * mid_t + control_offset_x;
        let control_y = y + dy * mid_t + control_offset_y;

        for i in 0..num_segments {
            let t = (i + 1) as f32 / num_segments as f32;

            // Quadratic Bezier curve for smooth random paths
            let inv_t = 1.0 - t;
            let next_x = inv_t * inv_t * x + 2.0 * inv_t * t * control_x + t * t * target_x;
            let next_y = inv_t * inv_t * y + 2.0 * inv_t * t * control_y + t * t * target_y;

            // Add small noise for organic feel
            let noise_x = (fastrand::f32() - 0.5) * 2.0;
            let noise_y = (fastrand::f32() - 0.5) * 2.0;
            let next_x = next_x + noise_x;
            let next_y = next_y + noise_y;

            if next_x < 0.0 || next_x >= width as f32 || next_y < 0.0 || next_y >= height as f32 {
                // If we hit a boundary, try to reach the actual edge
                let clamped_x = next_x.clamp(0.0, (width - 1) as f32);
                let clamped_y = next_y.clamp(0.0, (height - 1) as f32);

                segments.push(TendrilSegment {
                    x: current_x,
                    y: current_y,
                    end_x: clamped_x,
                    end_y: clamped_y,
                    intensity,
                    initial_y: current_y,
                    initial_end_y: clamped_y,
                });
                break;
            }

            segments.push(TendrilSegment {
                x: current_x,
                y: current_y,
                end_x: next_x,
                end_y: next_y,
                intensity,
                initial_y: current_y,
                initial_end_y: next_y,
            });

            // Branching
            if fastrand::f32() < 0.06 && generation < 2 && t > 0.3 && t < 0.7 {
                let branch_angle = if fastrand::bool() { 0.6 } else { -0.6 };
                let angle = dy.atan2(dx);
                let branch_dist = dist * (0.4 + fastrand::f32() * 0.3);
                let branch_target_x = next_x + (angle + branch_angle).cos() * branch_dist;
                let branch_target_y = next_y + (angle + branch_angle).sin() * branch_dist;

                Self::generate_tendril(
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

    fn get_color_at_distance(&self, dist_from_core: f32, total_length: f32) -> (u8, u8, u8) {
        // Pink at base, blue in middle, pink at tip
        let normalized_pos = dist_from_core / total_length;

        if normalized_pos < 0.15 {
            // Pink at base (near orb)
            let t = normalized_pos / 0.15;
            let pink = (255, 100, 150);
            let blue = (120, 150, 255);
            Self::blend_colors(pink, blue, t)
        } else if normalized_pos > 0.85 {
            // Pink at tip
            let t = (normalized_pos - 0.85) / 0.15;
            let blue = (120, 150, 255);
            let pink = (255, 100, 150);
            Self::blend_colors(blue, pink, t)
        } else {
            // Blue in middle
            (120, 150, 255)
        }
    }

    fn blend_colors(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
        (
            (c1.0 as f32 * (1.0 - t) + c2.0 as f32 * t) as u8,
            (c1.1 as f32 * (1.0 - t) + c2.1 as f32 * t) as u8,
            (c1.2 as f32 * (1.0 - t) + c2.2 as f32 * t) as u8,
        )
    }

    fn apply_opacity(color: (u8, u8, u8), opacity: f32) -> (u8, u8, u8) {
        // For additive glows: scale the color by opacity to represent amount of light added
        (
            (color.0 as f32 * opacity) as u8,
            (color.1 as f32 * opacity) as u8,
            (color.2 as f32 * opacity) as u8,
        )
    }

    fn add_colors(c1: (u8, u8, u8), c2: (u8, u8, u8)) -> (u8, u8, u8) {
        (
            (c1.0 as u16 + c2.0 as u16).min(255) as u8,
            (c1.1 as u16 + c2.1 as u16).min(255) as u8,
            (c1.2 as u16 + c2.2 as u16).min(255) as u8,
        )
    }

    fn update_target(
        &mut self,
        new_target_x: f32,
        new_target_y: f32,
        center_x: f32,
        center_y: f32,
        orb_radius: f32,
        width: usize,
        height: usize,
    ) {
        self.target_x = new_target_x;
        self.target_y = new_target_y;

        // Calculate angle from center to new target
        let dx = new_target_x - center_x;
        let dy = new_target_y - center_y;
        let base_angle = dy.atan2(dx);

        // Start from orb surface in direction of target
        let start_x = center_x + base_angle.cos() * orb_radius;
        let start_y = center_y + base_angle.sin() * orb_radius;

        // Regenerate segments
        self.segments.clear();
        Self::generate_tendril(
            &mut self.segments,
            start_x,
            start_y,
            new_target_x,
            new_target_y,
            1.0,
            0,
            width,
            height,
        );

        // Reset age to keep tendril fresh
        self.age = 0.0;
    }
}

pub struct PlasmaEffect {
    width: usize,
    height: usize,
    center_x: f32,
    center_y: f32,
    orb_radius: f32,
    tendrils: Vec<Tendril>,
    mouse_x: Option<f32>,
    mouse_y: Option<f32>,
    mouse_inactive_time: f32,
    time: f32,
    output_buf: Vec<u8>,
}

impl Effect for PlasmaEffect {
    fn new(width: usize, height: usize) -> Self {
        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;
        let orb_radius = 5.0;

        Self {
            width,
            height,
            center_x,
            center_y,
            orb_radius,
            tendrils: Vec::new(),
            mouse_x: None,
            mouse_y: None,
            mouse_inactive_time: 0.0,
            time: 0.0,
            output_buf: Vec::with_capacity(width * height * 25),
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        // Wrap time to prevent floating point precision issues
        if self.time > 10000.0 {
            self.time -= 10000.0;
        }
        self.mouse_inactive_time += dt;

        if self.mouse_inactive_time > 2.0 {
            self.mouse_x = None;
            self.mouse_y = None;
        }

        // Check if mouse is active
        let mouse_active = self.mouse_x.is_some() && self.mouse_y.is_some();

        if mouse_active {
            // When mouse is active: ONLY 1 tendril total (tracking mouse)
            // Remove all edge tendrils
            self.tendrils.retain(|t| t.is_mouse_tendril);

            // Ensure we have exactly 1 mouse tendril
            let has_mouse_tendril = self.tendrils.iter().any(|t| t.is_mouse_tendril);
            if !has_mouse_tendril {
                self.tendrils.push(Tendril::new(
                    self.center_x,
                    self.center_y,
                    self.orb_radius,
                    self.mouse_x,
                    self.mouse_y,
                    self.width,
                    self.height,
                    0, // Edge segment doesn't matter for mouse tendril
                    true, // Is a mouse tendril
                ));
            }
        } else {
            // When no mouse: Maintain 30 edge tendrils
            // Remove any mouse tendrils
            self.tendrils.retain(|t| !t.is_mouse_tendril);

            // Spawn edge tendrils to maintain 30
            while self.tendrils.len() < 30 {
                // Count tendrils per segment
                let mut segment_counts = [0u32; 12];
                for tendril in &self.tendrils {
                    segment_counts[tendril.edge_segment as usize] += 1;
                }

                // Find segment with minimum count
                let mut min_count = u32::MAX;
                let mut emptiest_segment = 0u8;
                for (i, &count) in segment_counts.iter().enumerate() {
                    if count < min_count {
                        min_count = count;
                        emptiest_segment = i as u8;
                    }
                }

                self.tendrils.push(Tendril::new(
                    self.center_x,
                    self.center_y,
                    self.orb_radius,
                    None,
                    None,
                    self.width,
                    self.height,
                    emptiest_segment,
                    false, // Not a mouse tendril
                ));
            }
        }

        // Update tendrils
        self.tendrils.retain_mut(|tendril| {
            tendril.age += dt;
            tendril.age < tendril.max_age
        });
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H");

        // Color pulsing effect - oscillates between more white and more pink
        let pulse = (self.time * 0.8).sin() * 0.5 + 0.5; // Oscillates between 0.0 and 1.0
        let bg_color = crate::get_bg_color();

        let mut glow_buffer = vec![(0.0f32, bg_color); self.width * self.height];

        // LAYER 1: Draw pinkish blur around core FIRST (background layer)
        let pink = (255, 100, 150);
        let blur_radius = self.orb_radius * 10.0; // Much wider blur
        let pulse_strength = 0.7 + pulse * 0.6; // Pulse between 0.7 and 1.3
        let max_glow_radius = self.orb_radius * 2.5;

        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 - self.center_x;
                let dy = y as f32 - self.center_y;
                let dist = (dx * dx + dy * dy).sqrt();

                // Draw blur in outer region (background layer - overlaps slightly with core to avoid gaps)
                if dist >= max_glow_radius && dist < blur_radius {
                    let blur_progress = (dist - max_glow_radius) / (blur_radius - max_glow_radius);
                    let falloff = 1.0 - blur_progress;
                    let exponential_falloff = falloff * falloff * falloff; // Cubic for quick dropoff

                    let idx = y * self.width + x;
                    let blur_opacity = exponential_falloff * 0.4 * pulse_strength;
                    // Blend pink with background color instead of scaling toward black
                    let blur_color = (
                        (bg_color.0 as f32 * (1.0 - blur_opacity) + pink.0 as f32 * blur_opacity) as u8,
                        (bg_color.1 as f32 * (1.0 - blur_opacity) + pink.1 as f32 * blur_opacity) as u8,
                        (bg_color.2 as f32 * (1.0 - blur_opacity) + pink.2 as f32 * blur_opacity) as u8,
                    );
                    let blur_intensity = exponential_falloff * 1.5 * pulse_strength;

                    // Always draw blur as background - it will be overridden by core and strong tendrils
                    glow_buffer[idx] = (blur_intensity, blur_color);
                }
            }
        }

        // LAYER 2: Draw tendrils
        for tendril in &self.tendrils {
            // Calculate opacity - only fade out, no fade in
            let opacity = if tendril.age > tendril.max_age - 0.08 {
                (tendril.max_age - tendril.age) / 0.08 // Fast fade out
            } else {
                1.0 // Full opacity (instant spawn)
            };

            // Calculate rise amount (hot gas rises over time)
            let rise_amount = tendril.age * 20.0; // Rise 20 pixels per second (more subtle)

            // Calculate sway motion for subtle movement
            let sway_time = (self.time * 2.5) + tendril.edge_segment as f32 * 0.5; // Offset per tendril, even faster sway

            // Calculate total path length for color gradient
            let mut total_length = 0.0;
            for segment in &tendril.segments {
                let dx = segment.end_x - segment.x;
                let dy = segment.end_y - segment.y;
                total_length += (dx * dx + dy * dy).sqrt();
            }

            let mut dist_from_core = 0.0;

            for segment in &tendril.segments {
                // Calculate segment length
                let seg_dx = segment.end_x - segment.x;
                let seg_dy = segment.end_y - segment.y;
                let seg_length = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt();

                // Apply rising motion to middle sections (not base or tip)
                // Calculate rise for START of segment
                let start_progress = dist_from_core / total_length;
                let start_rise_factor = if start_progress < 0.2 {
                    0.0
                } else if start_progress > 0.8 {
                    0.0
                } else {
                    let normalized = (start_progress - 0.2) / 0.6;
                    (normalized * std::f32::consts::PI).sin()
                };

                // Calculate rise for END of segment
                let end_progress = (dist_from_core + seg_length) / total_length;
                let end_rise_factor = if end_progress < 0.2 {
                    0.0
                } else if end_progress > 0.8 {
                    0.0
                } else {
                    let normalized = (end_progress - 0.2) / 0.6;
                    (normalized * std::f32::consts::PI).sin()
                };

                // Apply rise offset to both ends
                let y0 = (segment.initial_y - rise_amount * start_rise_factor).max(0.0);
                let y1 = (segment.initial_end_y - rise_amount * end_rise_factor).max(0.0);

                // Apply subtle sway motion - stronger at tips, weaker at base
                let sway_x_start = (sway_time * 1.2 + segment.x * 0.1).sin() * start_progress * 2.0;
                let sway_y_start = (sway_time * 0.9 + segment.y * 0.1).cos() * start_progress * 1.5;
                let sway_x_end = (sway_time * 1.2 + segment.end_x * 0.1).sin() * end_progress * 2.0;
                let sway_y_end = (sway_time * 0.9 + segment.end_y * 0.1).cos() * end_progress * 1.5;

                // Bresenham line drawing for no gaps
                let x0 = (segment.x + sway_x_start) as i32;
                let y0 = (y0 + sway_y_start) as i32;
                let x1 = (segment.end_x + sway_x_end) as i32;
                let y1 = (y1 + sway_y_end) as i32;

                if y0 < 0 || y1 < 0 {
                    dist_from_core += seg_length;
                    continue;
                }

                let dx = (x1 - x0).abs();
                let dy = (y1 - y0).abs();
                let sx = if x0 < x1 { 1 } else { -1 };
                let sy = if y0 < y1 { 1 } else { -1 };
                let mut err = dx - dy;

                let mut x = x0;
                let mut y = y0;

                let base_color = tendril.get_color_at_distance(dist_from_core, total_length);

                // Fade out main tendril at the tip to allow delta to show (gradual curve)
                let tip_fadeout = if end_progress > 0.70 {
                    let fade_progress = (end_progress - 0.70) / 0.30;
                    let smooth_fade = 1.0 - fade_progress;
                    smooth_fade * smooth_fade // Square for more gradual fade
                } else {
                    1.0
                };

                // Apply opacity to color so it fades out properly (darkens as it fades)
                let combined_opacity = opacity * tip_fadeout;
                let color = Tendril::apply_opacity(base_color, combined_opacity.max(0.3)); // Minimum 30% to avoid going too dark
                let base_intensity = segment.intensity * opacity * 0.5 * tip_fadeout; // Lower base intensity

                loop {
                    if x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
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
                        continue;
                    }

                    // Very thin tendril with multi-layer glow
                    let idx = y as usize * self.width + x as usize;
                    let center_glow = base_intensity * 3.0;
                    // Only draw if above minimum threshold to avoid black artifacts
                    if center_glow > 0.2 {
                        glow_buffer[idx] = (center_glow, color);
                    }

                    // Outer glow layer (5x5, very subtle)
                    for dy_offset in -2..=2 {
                        for dx_offset in -2..=2 {
                            if dy_offset == 0 && dx_offset == 0 {
                                continue; // Skip center (already drawn)
                            }
                            let ny = y + dy_offset;
                            let nx = x + dx_offset;
                            if ny >= 0 && ny < self.height as i32 && nx >= 0 && nx < self.width as i32 {
                                let idx = ny as usize * self.width + nx as usize;
                                let is_inner = dy_offset.abs() <= 1 && dx_offset.abs() <= 1;

                                if is_inner {
                                    // Inner glow (3x3) - soft bloom, fades with tendril
                                    // Use ADDITIVE blending instead of replacing - very subtle
                                    let glow_opacity = 0.08 * combined_opacity.max(0.2); // Much weaker glow
                                    let glow_color = Tendril::apply_opacity(base_color, glow_opacity);
                                    if glow_opacity > 0.02 {
                                        let existing_color = glow_buffer[idx].1;
                                        let new_color = Tendril::add_colors(existing_color, glow_color);
                                        // Keep intensity as max of the two
                                        let new_intensity = glow_buffer[idx].0.max(base_intensity * 0.15);
                                        glow_buffer[idx] = (new_intensity, new_color);
                                    }
                                } else {
                                    // Outer glow (5x5 edge) - very soft bloom, fades with tendril
                                    // Use ADDITIVE blending instead of replacing - extremely subtle
                                    let glow_opacity = 0.04 * combined_opacity.max(0.15); // Much weaker glow
                                    let glow_color = Tendril::apply_opacity(base_color, glow_opacity);
                                    if glow_opacity > 0.01 {
                                        let existing_color = glow_buffer[idx].1;
                                        let new_color = Tendril::add_colors(existing_color, glow_color);
                                        // Keep intensity as max of the two
                                        let new_intensity = glow_buffer[idx].0.max(base_intensity * 0.08);
                                        glow_buffer[idx] = (new_intensity, new_color);
                                    }
                                }
                            }
                        }
                    }

                    // Delta effect at tips - gradual dispersion like a river delta
                    if end_progress > 0.70 {
                        // Calculate progress within the delta zone (0.0 at start, 1.0 at edge)
                        let delta_progress = (end_progress - 0.70) / 0.30;
                        // Spread grows gradually from 1 to 7 pixels as we move through delta zone
                        let spread_radius = (1.0 + delta_progress * 6.0) as i32;

                        // Bright pink for delta
                        let pink = (255, 100, 150);

                        // Only draw if we have some spread
                        if spread_radius > 0 {
                            // Disperse in a spreading pattern
                            for dy_spread in -spread_radius..=spread_radius {
                                for dx_spread in -spread_radius..=spread_radius {
                                    let ny = y + dy_spread;
                                    let nx = x + dx_spread;

                                    if ny >= 0 && ny < self.height as i32 && nx >= 0 && nx < self.width as i32 {
                                        let spread_dist = ((dx_spread * dx_spread + dy_spread * dy_spread) as f32).sqrt();
                                        if spread_dist <= spread_radius as f32 && spread_dist > 0.5 {
                                            let idx = ny as usize * self.width + nx as usize;

                                            // Exponential falloff - super intense in center, falls off quickly
                                            let linear_falloff = 1.0 - (spread_dist / spread_radius as f32);
                                            let falloff = linear_falloff * linear_falloff * linear_falloff; // Cubic for sharp dropoff
                                            // Intensity grows with delta progress
                                            let delta_brightness = falloff * delta_progress * 1.8;
                                            let delta_opacity = falloff * delta_progress * 0.5; // Keep delta bright, don't fade with tendril (reduced slightly)

                                            let delta_color = Tendril::apply_opacity(pink, delta_opacity);
                                            let delta_glow = segment.intensity * delta_brightness * opacity;

                                            // Use ADDITIVE blending for delta effect
                                            if delta_glow > 0.15 {
                                                let existing_color = glow_buffer[idx].1;
                                                let new_color = Tendril::add_colors(existing_color, delta_color);
                                                // Keep intensity as max of the two
                                                let new_intensity = glow_buffer[idx].0.max(delta_glow);
                                                glow_buffer[idx] = (new_intensity, new_color);
                                            }
                                        }
                                    }
                                }
                            }
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

                dist_from_core += seg_length;
            }
        }

        // LAYER 3: Draw central orb on top of everything (pink core with white glow)
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 - self.center_x;
                let dy = y as f32 - self.center_y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < max_glow_radius {
                    let base_intensity = ((max_glow_radius - dist) / max_glow_radius * 5.5).min(5.5);
                    // Shift color index based on pulse - pulse towards pink (higher index) or white (lower index)
                    let color_shift = pulse * 1.5; // Shift up to 1.5 color indices
                    let shifted_intensity = base_intensity + color_shift;

                    let idx = y * self.width + x;
                    let color_idx = (shifted_intensity as usize).min(CORE_COLORS.len() - 1);
                    glow_buffer[idx] = (base_intensity, CORE_COLORS[color_idx]); // Always override
                }
            }
        }

        // LAYER 4: Add tendril glow on top of core (additive blending)
        for tendril in &self.tendrils {
            let opacity = if tendril.age > tendril.max_age - 0.08 {
                (tendril.max_age - tendril.age) / 0.08
            } else {
                1.0
            };

            let rise_amount = tendril.age * 20.0;
            let sway_time = (self.time * 2.5) + tendril.edge_segment as f32 * 0.5;

            let mut total_length = 0.0;
            for segment in &tendril.segments {
                let dx = segment.end_x - segment.x;
                let dy = segment.end_y - segment.y;
                total_length += (dx * dx + dy * dy).sqrt();
            }

            let mut dist_from_core = 0.0;

            for segment in &tendril.segments {
                let seg_dx = segment.end_x - segment.x;
                let seg_dy = segment.end_y - segment.y;
                let seg_length = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt();

                let start_progress = dist_from_core / total_length;
                let end_progress = (dist_from_core + seg_length) / total_length;

                let start_rise_factor = if start_progress < 0.2 {
                    0.0
                } else if start_progress > 0.8 {
                    0.0
                } else {
                    let normalized = (start_progress - 0.2) / 0.6;
                    (normalized * std::f32::consts::PI).sin()
                };

                let end_rise_factor = if end_progress < 0.2 {
                    0.0
                } else if end_progress > 0.8 {
                    0.0
                } else {
                    let normalized = (end_progress - 0.2) / 0.6;
                    (normalized * std::f32::consts::PI).sin()
                };

                let y0 = (segment.initial_y - rise_amount * start_rise_factor).max(0.0);
                let y1 = (segment.initial_end_y - rise_amount * end_rise_factor).max(0.0);

                let sway_x_start = (sway_time * 1.2 + segment.x * 0.1).sin() * start_progress * 2.0;
                let sway_y_start = (sway_time * 0.9 + segment.y * 0.1).cos() * start_progress * 1.5;
                let sway_x_end = (sway_time * 1.2 + segment.end_x * 0.1).sin() * end_progress * 2.0;
                let sway_y_end = (sway_time * 0.9 + segment.end_y * 0.1).cos() * end_progress * 1.5;

                let x0 = (segment.x + sway_x_start) as i32;
                let y0 = (y0 + sway_y_start) as i32;
                let x1 = (segment.end_x + sway_x_end) as i32;
                let y1 = (y1 + sway_y_end) as i32;

                if y0 < 0 || y1 < 0 {
                    dist_from_core += seg_length;
                    continue;
                }

                let dx = (x1 - x0).abs();
                let dy = (y1 - y0).abs();
                let sx = if x0 < x1 { 1 } else { -1 };
                let sy = if y0 < y1 { 1 } else { -1 };
                let mut err = dx - dy;

                let mut x = x0;
                let mut y = y0;

                let base_color = tendril.get_color_at_distance(dist_from_core, total_length);

                let tip_fadeout = if end_progress > 0.70 {
                    let fade_progress = (end_progress - 0.70) / 0.30;
                    let smooth_fade = 1.0 - fade_progress;
                    smooth_fade * smooth_fade
                } else {
                    1.0
                };

                let combined_opacity = opacity * tip_fadeout;

                loop {
                    if x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
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
                        continue;
                    }

                    // Add glow around tendril using additive blending
                    for dy_offset in -2..=2 {
                        for dx_offset in -2..=2 {
                            if dy_offset == 0 && dx_offset == 0 {
                                continue; // Skip center
                            }
                            let ny = y + dy_offset;
                            let nx = x + dx_offset;
                            if ny >= 0 && ny < self.height as i32 && nx >= 0 && nx < self.width as i32 {
                                let idx = ny as usize * self.width + nx as usize;
                                let is_inner = dy_offset.abs() <= 1 && dx_offset.abs() <= 1;

                                if is_inner {
                                    let glow_opacity = 0.08 * combined_opacity.max(0.2);
                                    let glow_color = Tendril::apply_opacity(base_color, glow_opacity);
                                    if glow_opacity > 0.02 {
                                        let existing_color = glow_buffer[idx].1;
                                        let new_color = Tendril::add_colors(existing_color, glow_color);
                                        glow_buffer[idx].1 = new_color;
                                    }
                                } else {
                                    let glow_opacity = 0.04 * combined_opacity.max(0.15);
                                    let glow_color = Tendril::apply_opacity(base_color, glow_opacity);
                                    if glow_opacity > 0.01 {
                                        let existing_color = glow_buffer[idx].1;
                                        let new_color = Tendril::add_colors(existing_color, glow_color);
                                        glow_buffer[idx].1 = new_color;
                                    }
                                }
                            }
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

                dist_from_core += seg_length;
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

                let (_top_intensity, top_color) = glow_buffer[top_idx];
                let (_bot_intensity, bot_color) = glow_buffer[bot_idx];

                // No need to check intensity threshold - we initialized with bg_color
                // and all additive blending preserves proper colors

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

    fn handle_event(&mut self, event: &Event) {
        if let Event::Mouse(MouseEvent { kind, column, row, .. }) = event {
            if let MouseEventKind::Moved = kind {
                let new_mouse_x = *column as f32;
                let new_mouse_y = *row as f32 * 2.0;

                self.mouse_x = Some(new_mouse_x);
                self.mouse_y = Some(new_mouse_y);
                self.mouse_inactive_time = 0.0;

                // Update existing mouse tendril target if it exists
                if let Some(mouse_tendril) = self.tendrils.iter_mut().find(|t| t.is_mouse_tendril) {
                    mouse_tendril.update_target(
                        new_mouse_x,
                        new_mouse_y,
                        self.center_x,
                        self.center_y,
                        self.orb_radius,
                        self.width,
                        self.height,
                    );
                }
            }
        }
    }
}
