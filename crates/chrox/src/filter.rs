use chromoxide::Oklch;
use phf::phf_map;

static FILTERS: phf::Map<&'static str, fn(Oklch) -> String> = phf_map! {};

// TODO: define filters (e.g., hex, rgb, hsl)
