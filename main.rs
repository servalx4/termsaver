use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::env;
use std::io::{stdout, BufWriter};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

mod effects;
use effects::Effect;

static BG_COLOR: OnceLock<(u8, u8, u8)> = OnceLock::new();

pub fn get_bg_color() -> (u8, u8, u8) {
    *BG_COLOR.get().unwrap_or(&(0, 0, 0))
}

fn print_usage() {
    eprintln!("termsaver - Terminal screensaver with various effects");
    eprintln!();
    eprintln!("Usage: termsaver [EFFECT] [OPTIONS]");
    eprintln!();
    eprintln!("Effects:");
    eprintln!("  fire      Fire screensaver (default)");
    eprintln!("  thunder   Realistic branching lightning");
    eprintln!("  plasma    Interactive plasma globe with mouse");
    eprintln!("  fireworks Colorful fireworks display");
    eprintln!("  lavalamp  Smooth metaball lava lamp animation");
    eprintln!("  aurora    Aurora borealis with smooth wave-like patterns");
    eprintln!("  clouds    Realistic clouds with multiple types and volumetric shading");
    eprintln!("  biolum    Deep sea bioluminescence with jellyfish and schooling fish");
    eprintln!("  star      Realistic star with accurate stellar classification and physics");
    eprintln!("  gol       Conway's Game of Life with randomized multi-state rules");
    eprintln!("            Controls: 0-8 = survival, !@#$%^&*() = birth, +/- = states, R = random, click = cycle cell");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --bg-color RRGGBB  Set background color as hex (e.g., --bg-color 1a1b26)");
    eprintln!();
    eprintln!("Press 'q', ESC, or Ctrl+C to exit");
}

fn run_effect<E: Effect>() -> std::io::Result<()> {
    let stdout = stdout();
    let mut stdout = BufWriter::with_capacity(1024 * 64, stdout);

    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All), EnableMouseCapture)?;

    let (cols, rows) = terminal::size()?;
    let mut effect = E::new(cols as usize, rows as usize * 2);

    let mut last_frame = Instant::now();
    let mut accumulator = 0.0f32;
    const FIXED_DT: f32 = 1.0 / 60.0;

    loop {
        if event::poll(Duration::from_millis(1))? {
            let event = event::read()?;
            match &event {
                Event::Key(key_event) => {
                    if key_event.code == KeyCode::Char('q')
                        || key_event.code == KeyCode::Esc
                        || (key_event.code == KeyCode::Char('c')
                            && key_event.modifiers.contains(event::KeyModifiers::CONTROL))
                    {
                        break;
                    }
                    // Pass non-exit key events to the effect
                    effect.handle_event(&event);
                }
                Event::Resize(cols, rows) => {
                    effect = E::new(*cols as usize, *rows as usize * 2);
                    execute!(stdout, Clear(ClearType::All))?;
                }
                _ => {
                    effect.handle_event(&event);
                }
            }
        }

        let now = Instant::now();
        let frame_time = now.duration_since(last_frame).as_secs_f32();
        last_frame = now;

        accumulator += frame_time;
        if accumulator > FIXED_DT * 3.0 {
            accumulator = FIXED_DT * 3.0;
        }

        while accumulator >= FIXED_DT {
            effect.update(FIXED_DT);
            accumulator -= FIXED_DT;
        }

        effect.render(&mut stdout)?;
    }

    execute!(stdout, Show, LeaveAlternateScreen, DisableMouseCapture)?;
    terminal::disable_raw_mode()?;

    Ok(())
}

fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some((r, g, b))
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut effect_name = "fire";
    let mut bg_color: Option<(u8, u8, u8)> = None;

    // Parse arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--bg-color" => {
                if i + 1 < args.len() {
                    if let Some(color) = parse_hex_color(&args[i + 1]) {
                        bg_color = Some(color);
                        i += 2;
                    } else {
                        eprintln!("Invalid hex color: {}", args[i + 1]);
                        eprintln!("Expected format: RRGGBB (e.g., 1a1b26)");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("--bg-color requires a hex color value");
                    std::process::exit(1);
                }
            }
            "help" | "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            arg => {
                if !arg.starts_with('-') {
                    effect_name = arg;
                    i += 1;
                } else {
                    eprintln!("Unknown option: {}", arg);
                    eprintln!();
                    print_usage();
                    std::process::exit(1);
                }
            }
        }
    }

    // Set background color if provided
    if let Some(color) = bg_color {
        let _ = BG_COLOR.set(color);
    }

    match effect_name {
        "fire" => run_effect::<effects::fire::FireEffect>(),
        "thunder" => run_effect::<effects::thunder::ThunderEffect>(),
        "plasma" => run_effect::<effects::plasma::PlasmaEffect>(),
        "fireworks" => run_effect::<effects::fireworks::FireworksEffect>(),
        "lavalamp" => run_effect::<effects::lavalamp::LavaLampEffect>(),
        "aurora" => run_effect::<effects::aurora::AuroraEffect>(),
        "clouds" => run_effect::<effects::clouds::CloudEffect>(),
        "biolum" => run_effect::<effects::bioluminescence::BioluminescenceEffect>(),
        "star" => run_effect::<effects::star::StarEffect>(),
        "gol" => run_effect::<effects::gameoflife::GameOfLifeEffect>(),
        _ => {
            eprintln!("Unknown effect: {}", effect_name);
            eprintln!();
            print_usage();
            std::process::exit(1);
        }
    }
}
