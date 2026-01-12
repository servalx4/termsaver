use crossterm::event::Event;
use std::io::{BufWriter, Stdout};

pub mod fire;
pub mod thunder;
pub mod plasma;
pub mod fireworks;
pub mod lavalamp;
pub mod gameoflife;
pub mod aurora;
pub mod clouds;
pub mod bioluminescence;
pub mod star;

pub trait Effect {
    fn new(width: usize, height: usize) -> Self
    where
        Self: Sized;
    fn update(&mut self, dt: f32);
    fn render(&mut self, stdout: &mut BufWriter<Stdout>) -> std::io::Result<()>;
    fn handle_event(&mut self, _event: &Event) {}
}
