use chromoxide::Oklch;
use chromoxide::convert::oklab_to_linear_srgb;
use phf::phf_map;

pub type FilterFn = fn(Oklch) -> String;

static FILTERS: phf::Map<&'static str, FilterFn> = phf_map! {
    "hex" => hex,
    "hex_raw" => hex_raw,
    "rgb" => rgb,
    "rgb_raw" => rgb_raw,
    "oklch" => oklch_css,
    "oklch_raw" => oklch_raw,
    "l" => lightness,
    "c" => chroma,
    "h" => hue_radians,
    "hdeg" => hue_degrees,
};

pub fn get(name: &str) -> Option<FilterFn> {
    FILTERS.get(name).copied()
}

pub fn apply(name: &str, color: Oklch) -> Option<String> {
    get(name).map(|filter| filter(color))
}

pub fn is_supported(name: &str) -> bool {
    FILTERS.contains_key(name)
}

fn hex(color: Oklch) -> String {
    format!("#{}", hex_raw(color))
}

fn hex_raw(color: Oklch) -> String {
    let (r, g, b) = srgb_u8(color);
    format!("{r:02x}{g:02x}{b:02x}")
}

fn rgb(color: Oklch) -> String {
    let (r, g, b) = srgb_u8(color);
    format!("rgb({r}, {g}, {b})")
}

fn rgb_raw(color: Oklch) -> String {
    let (r, g, b) = srgb_u8(color);
    format!("{r} {g} {b}")
}

fn oklch_css(color: Oklch) -> String {
    let hdeg = hue_to_degrees(color.h);
    format!("oklch({:.6} {:.6} {:.2}deg)", color.l, color.c, hdeg)
}

fn oklch_raw(color: Oklch) -> String {
    format!(
        "{:.6} {:.6} {:.6}",
        color.l,
        color.c,
        normalize_hue(color.h)
    )
}

fn lightness(color: Oklch) -> String {
    format!("{:.6}", color.l)
}

fn chroma(color: Oklch) -> String {
    format!("{:.6}", color.c)
}

fn hue_radians(color: Oklch) -> String {
    format!("{:.6}", normalize_hue(color.h))
}

fn hue_degrees(color: Oklch) -> String {
    format!("{:.2}", hue_to_degrees(color.h))
}

fn srgb_u8(color: Oklch) -> (u8, u8, u8) {
    let linear = oklab_to_linear_srgb(color.to_oklab());
    let r = to_u8(linear_to_srgb_channel(linear.r));
    let g = to_u8(linear_to_srgb_channel(linear.g));
    let b = to_u8(linear_to_srgb_channel(linear.b));
    (r, g, b)
}

fn to_u8(channel: f64) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn linear_to_srgb_channel(channel: f64) -> f64 {
    if channel <= 0.003_130_8 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

fn normalize_hue(hue: f64) -> f64 {
    let mut h = hue % std::f64::consts::TAU;
    if h < 0.0 {
        h += std::f64::consts::TAU;
    }
    h
}

fn hue_to_degrees(hue: f64) -> f64 {
    normalize_hue(hue).to_degrees()
}

#[cfg(test)]
mod tests {
    use chromoxide::Oklch;

    use super::{apply, is_supported};

    #[test]
    fn builtin_filters_are_registered() {
        for filter in ["hex", "rgb", "oklch", "l", "c", "h", "hdeg"] {
            assert!(is_supported(filter));
        }
    }

    #[test]
    fn hex_filter_formats_white_and_black() {
        let white = Oklch {
            l: 1.0,
            c: 0.0,
            h: 0.0,
        };
        let black = Oklch {
            l: 0.0,
            c: 0.0,
            h: 0.0,
        };

        assert_eq!(apply("hex", white), Some("#ffffff".to_string()));
        assert_eq!(apply("hex", black), Some("#000000".to_string()));
    }

    #[test]
    fn hue_filter_wraps_negative_values() {
        let color = Oklch {
            l: 0.5,
            c: 0.1,
            h: -std::f64::consts::PI,
        };

        assert_eq!(apply("hdeg", color), Some("180.00".to_string()));
    }
}
