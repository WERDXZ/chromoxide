use chromoxide::Oklch;

pub mod user;

pub trait Palette {
    fn id() -> String;
    fn name() -> String;
    fn color(name: &str) -> Option<Oklch>;
}
