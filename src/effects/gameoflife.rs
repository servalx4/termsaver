use super::Effect;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use std::io::{BufWriter, Stdout, Write};

pub struct GameOfLifeEffect {
    width: usize,
    height: usize,
    cells: Vec<u8>,      // Cell states (0 = dead, 1 = alive, 2+ = dying)
    next_cells: Vec<u8>,
    output_buf: Vec<u8>,
    survival_rules: Vec<u8>, // Neighbor counts that keep a cell alive
    birth_rules: Vec<u8>,    // Neighbor counts that birth a new cell
    update_counter: f32,
    update_interval: f32, // Seconds between updates
    current_color: (u8, u8, u8),
    target_color: (u8, u8, u8),
    alive_color: (u8, u8, u8),
    color_transition: f32,
    num_states: u8, // Total number of states (2-10)
}

impl Effect for GameOfLifeEffect {
    fn new(width: usize, height: usize) -> Self {
        let cell_count = width * height;
        let num_states = 2 + fastrand::u8(0..9); // Random 2-10 states
        let mut cells = vec![0u8; cell_count];

        // Random initial state (15% alive at max state)
        for cell in cells.iter_mut() {
            if fastrand::f32() < 0.15 {
                *cell = num_states - 1;
            }
        }

        let (survival_rules, birth_rules) = Self::random_rules();

        let current_color = Self::random_color();
        let target_color = Self::random_color();

        Self {
            width,
            height,
            cells,
            next_cells: vec![0u8; cell_count],
            output_buf: Vec::with_capacity(width * height * 25),
            survival_rules,
            birth_rules,
            update_counter: 0.0,
            update_interval: 0.1, // 10 updates per second
            current_color,
            target_color,
            alive_color: current_color,
            color_transition: 0.0,
            num_states,
        }
    }

    fn update(&mut self, dt: f32) {
        self.update_counter += dt;

        // Smooth color transitions
        self.color_transition += dt * 0.02; // Takes ~50 seconds per transition

        if self.color_transition >= 1.0 {
            // Reached target color, pick a new target
            self.current_color = self.target_color;
            self.target_color = Self::random_color();
            self.color_transition = 0.0;
        }

        // Interpolate between current and target to get display color
        let t = self.color_transition;
        let r = (self.current_color.0 as f32 * (1.0 - t) + self.target_color.0 as f32 * t) as u8;
        let g = (self.current_color.1 as f32 * (1.0 - t) + self.target_color.1 as f32 * t) as u8;
        let b = (self.current_color.2 as f32 * (1.0 - t) + self.target_color.2 as f32 * t) as u8;
        self.alive_color = (r, g, b);

        if self.update_counter >= self.update_interval {
            self.update_counter = 0.0;
            self.step_generation();
        }
    }

    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()> {
        self.output_buf.clear();
        self.output_buf.extend_from_slice(b"\x1b[H"); // Move to home

        let bg_color = crate::get_bg_color();

        // Build rule string
        let survival_str: String = self.survival_rules.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("");
        let birth_str: String = self.birth_rules.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("");
        let rule_text = format!("B{}/S{}/{}", birth_str, survival_str, self.num_states);

        let mut prev_top: (u8, u8, u8) = (255, 255, 255);
        let mut prev_bot: (u8, u8, u8) = (255, 255, 255);

        for y in (0..self.height).step_by(2) {
            for x in 0..self.width {
                let top_state = self.cells[y * self.width + x];
                let bot_state = if y + 1 < self.height {
                    self.cells[(y + 1) * self.width + x]
                } else {
                    0
                };

                let top = self.state_to_color(top_state, bg_color);
                let bot = self.state_to_color(bot_state, bg_color);

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

        // Draw rule text in top left (overlay)
        write!(self.output_buf, "\x1b[1;1H\x1b[38;2;255;255;255m\x1b[48;2;0;0;0m {rule_text} \x1b[0m")?;

        stdout.write_all(&self.output_buf)?;
        stdout.flush()?;
        Ok(())
    }

    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::Mouse(mouse_event) => {
                match mouse_event.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Toggle cell at mouse position
                        let x = mouse_event.column as usize;
                        let y = mouse_event.row as usize * 2; // Account for half-block rendering

                        if x < self.width && y < self.height {
                            let idx = y * self.width + x;
                            // Cycle through states
                            self.cells[idx] = (self.cells[idx] + 1) % self.num_states;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Right) => {
                        // Right click resets
                        self.reset();
                    }
                    _ => {}
                }
            }
            Event::Key(key_event) => {
                // Number keys 0-8 to toggle survival rules
                // Shift+0-8 (!@#$%^&*()) to toggle birth rules
                // 'r' to randomize rules
                match key_event.code {
                    KeyCode::Char('r') => {
                        let (survival, birth) = Self::random_rules();
                        self.survival_rules = survival;
                        self.birth_rules = birth;
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        // Increase state count
                        if self.num_states < 20 {
                            self.num_states += 1;
                        }
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        // Decrease state count
                        if self.num_states > 2 {
                            self.num_states -= 1;
                        }
                    }
                    KeyCode::Char(c @ '0'..='8') => {
                        let num = c.to_digit(10).unwrap() as u8;
                        if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                            // Toggle birth rule (Shift+number)
                            if let Some(pos) = self.birth_rules.iter().position(|&x| x == num) {
                                self.birth_rules.remove(pos);
                            } else {
                                self.birth_rules.push(num);
                                self.birth_rules.sort();
                            }
                        } else {
                            // Toggle survival rule (number)
                            if let Some(pos) = self.survival_rules.iter().position(|&x| x == num) {
                                self.survival_rules.remove(pos);
                            } else {
                                self.survival_rules.push(num);
                                self.survival_rules.sort();
                            }
                        }
                    }
                    // Handle shifted number keys (!, @, #, $, %, ^, &, *, ()
                    KeyCode::Char(c) => {
                        let num = match c {
                            ')' => Some(0),
                            '!' => Some(1),
                            '@' => Some(2),
                            '#' => Some(3),
                            '$' => Some(4),
                            '%' => Some(5),
                            '^' => Some(6),
                            '&' => Some(7),
                            '*' => Some(8),
                            _ => None,
                        };

                        if let Some(num) = num {
                            // Toggle birth rule (shifted symbols)
                            if let Some(pos) = self.birth_rules.iter().position(|&x| x == num) {
                                self.birth_rules.remove(pos);
                            } else {
                                self.birth_rules.push(num);
                                self.birth_rules.sort();
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl GameOfLifeEffect {
    fn random_rules() -> (Vec<u8>, Vec<u8>) {
        // Generate completely random rules
        let mut survival = Vec::new();
        let mut birth = Vec::new();

        for i in 0..9 {
            if fastrand::f32() < 0.3 {
                survival.push(i);
            }
            if fastrand::f32() < 0.3 {
                birth.push(i);
            }
        }

        // Ensure at least one rule exists
        if survival.is_empty() && birth.is_empty() {
            survival.push(2);
            survival.push(3);
            birth.push(3);
        }

        (survival, birth)
    }

    fn random_color() -> (u8, u8, u8) {
        let hue = fastrand::f32();
        let s = 0.6 + fastrand::f32() * 0.4;
        let v = 0.7 + fastrand::f32() * 0.3;

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

    fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        let max_state = self.num_states - 1;

        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (x as i32 + dx + self.width as i32) % self.width as i32;
                let ny = (y as i32 + dy + self.height as i32) % self.height as i32;

                // Count neighbors that are in max state (fully alive)
                if self.cells[ny as usize * self.width + nx as usize] == max_state {
                    count += 1;
                }
            }
        }

        count
    }

    fn step_generation(&mut self) {
        let max_state = self.num_states - 1;

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let state = self.cells[idx];
                let neighbors = self.count_neighbors(x, y);

                self.next_cells[idx] = if state == 0 {
                    // Dead cell - check birth rules
                    if self.birth_rules.contains(&neighbors) {
                        max_state // Birth into max state (fully alive)
                    } else {
                        0 // Stay dead
                    }
                } else if state == max_state {
                    // Fully alive cell - check survival rules
                    if self.survival_rules.contains(&neighbors) {
                        max_state // Survive
                    } else {
                        if self.num_states > 2 {
                            max_state - 1 // Die into first dying state
                        } else {
                            0 // Die immediately if only 2 states
                        }
                    }
                } else {
                    // Dying state - decay toward dead
                    if state > 0 {
                        state - 1
                    } else {
                        0
                    }
                };
            }
        }

        std::mem::swap(&mut self.cells, &mut self.next_cells);
    }

    fn reset(&mut self) {
        // Randomize rules and state count first
        let (survival, birth) = Self::random_rules();
        self.survival_rules = survival;
        self.birth_rules = birth;
        self.num_states = 2 + fastrand::u8(0..9); // Random 2-10 states

        // Randomize cells again (spawn at max state)
        let max_state = self.num_states - 1;
        for cell in self.cells.iter_mut() {
            *cell = if fastrand::f32() < 0.15 { max_state } else { 0 };
        }

        // Reset color transition
        self.current_color = self.alive_color;
        self.target_color = Self::random_color();
        self.color_transition = 0.0;
    }

    fn state_to_color(&self, state: u8, bg_color: (u8, u8, u8)) -> (u8, u8, u8) {
        if state == 0 {
            // Dead = background (0% brightness)
            bg_color
        } else {
            // Brightness proportional to state
            // State 1 = 1/(N-1) brightness
            // State N-1 = (N-1)/(N-1) = 100% brightness
            let max_state = (self.num_states - 1) as f32;
            let brightness = state as f32 / max_state;

            let r = (bg_color.0 as f32 * (1.0 - brightness) + self.alive_color.0 as f32 * brightness) as u8;
            let g = (bg_color.1 as f32 * (1.0 - brightness) + self.alive_color.1 as f32 * brightness) as u8;
            let b = (bg_color.2 as f32 * (1.0 - brightness) + self.alive_color.2 as f32 * brightness) as u8;
            (r, g, b)
        }
    }
}
