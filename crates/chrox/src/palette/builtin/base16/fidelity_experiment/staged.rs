//! Experimental source snapshot followed by explicit text/gamut projection.
use chromoxide::{
    Oklch,
    convert::{contrast_ratio, oklab_to_linear_srgb},
};

use super::semantics::{Assignment, MIN_CONTRAST, ROLES, Role, TEXT_SLOTS};

#[derive(serde::Serialize)]
pub(super) struct ScenePalette {
    pub colors: Vec<Oklch>,
    pub samples: Vec<chromoxide::WeightedSample>,
}

#[derive(serde::Serialize)]
pub(super) struct TextProjection {
    slot: usize,
    target: Oklch,
    color: Oklch,
}

#[derive(serde::Serialize)]
pub(super) struct ThemePalette {
    pub colors: Vec<Oklch>,
    projections: Vec<TextProjection>,
    text_slots: Vec<usize>,
    decorative_slot: Option<usize>,
}

pub(super) fn adapt(
    scene: &ScenePalette,
    assignments: &[Assignment],
    decorate: bool,
) -> Result<ThemePalette, String> {
    if scene.colors.len() != 16 || assignments.len() != 8 {
        return Err(
            "staged adaptation requires 16 colors and eight source/semantic assignments".into(),
        );
    }
    let mut colors: Vec<_> = scene
        .colors
        .iter()
        .copied()
        .map(gamut_at_fixed_lightness)
        .collect();
    let background = colors[0];
    let mut projections = Vec::new();
    for slot in TEXT_SLOTS {
        let role = ROLES.into_iter().find(|r| r.slot == slot);
        let target = assignments
            .iter()
            .find(|a| a.slot == slot)
            .map_or(scene.colors[slot], |a| {
                // Allocation supplies the role hue; restore actual source L rather
                // than the fixed recipe's pre-clamped lightness target.
                Oklch {
                    l: a.source.as_ref().map_or(a.target.l, |s| s.lab.l),
                    ..a.target
                }
            });
        let color = project_text(target, background, role)?;
        colors[slot] = color;
        projections.push(TextProjection {
            slot,
            target,
            color,
        });
    }
    if decorate {
        reserve_decoration(&mut colors, &scene.samples);
    }
    Ok(ThemePalette {
        colors,
        projections,
        text_slots: TEXT_SLOTS.to_vec(),
        decorative_slot: decorate.then_some(4),
    })
}

fn gamut_at_fixed_lightness(source: Oklch) -> Oklch {
    if in_gamut(source) {
        return source;
    }
    let mut color = Oklch { c: 0.0, ..source };
    let mut upper = source.c;
    for _ in 0..32 {
        let candidate = Oklch {
            c: 0.5 * (color.c + upper),
            ..source
        };
        if in_gamut(candidate) {
            color = candidate;
        } else {
            upper = candidate.c;
        }
    }
    color
}

fn reserve_decoration(colors: &mut [Oklch], samples: &[chromoxide::WeightedSample]) {
    // Slot 4 is explicitly decorative in this ablation. Its source coverage
    // can improve without asking a dark source to pass a text contrast check.
    let others: Vec<_> = colors
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 4)
        .map(|(_, &c)| displayed_lab(c))
        .collect();
    let mass: f64 = samples.iter().map(|s| s.weight).sum();
    let cost = |color| {
        let lab = displayed_lab(color);
        samples
            .iter()
            .map(|s| {
                s.weight
                    * others
                        .iter()
                        .map(|&c| c.distance2(s.lab))
                        .fold(lab.distance2(s.lab), f64::min)
            })
            .sum::<f64>()
            / mass
    };
    // Reuse the selector's merged mass/noise eligibility. Raw samples would
    // give even one isolated pixel a decorative slot once gray is covered.
    let mut candidates: Vec<_> = super::anchors::source_anchors(samples, 8)
        .into_iter()
        .map(|s| s.lab)
        .collect();
    candidates.sort_by(|a, b| {
        a.l.total_cmp(&b.l)
            .then(a.a.total_cmp(&b.a))
            .then(a.b.total_cmp(&b.b))
    });
    let mut best = cost(colors[4]);
    for lab in candidates {
        let candidate = gamut_at_fixed_lightness(lab.to_oklch());
        let score = cost(candidate);
        if score < best - 1e-12 {
            colors[4] = candidate;
            best = score;
        }
    }
}

fn luminance(color: Oklch) -> f64 {
    let (r, g, b) = crate::filter::srgb_u8(color);
    let linear = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn in_gamut(color: Oklch) -> bool {
    let rgb = oklab_to_linear_srgb(color.to_oklab());
    [rgb.r, rgb.g, rgb.b]
        .iter()
        .all(|v| (-1e-9..=1.0 + 1e-9).contains(v))
}

fn project_text(source: Oklch, background: Oklch, role: Option<Role>) -> Result<Oklch, String> {
    let bg = luminance(background);
    let valid = |color: Oklch| {
        in_gamut(color)
            && contrast_ratio(luminance(color), bg) >= MIN_CONTRAST + 0.1
            && role.is_none_or(|r| {
                let displayed = displayed_lab(color).to_oklch();
                r.hue().contains(displayed.h) && displayed.c >= 0.03
            })
    };
    // Search upward in L, holding C/H fixed. The grid bounds the first-entry
    // search resolution to <= 1/1024 in L; refinement retains a valid endpoint.
    let at_chroma = |c| {
        let mut previous = source.l;
        for step in 0..=1024 {
            let mut color = Oklch {
                l: source.l + (1.0 - source.l) * f64::from(step) / 1024.0,
                c,
                h: source.h,
            };
            if valid(color) {
                let mut lower = previous;
                for _ in 0..24 {
                    let mid = 0.5 * (lower + color.l);
                    let candidate = Oklch { l: mid, ..color };
                    if valid(candidate) {
                        color = candidate;
                    } else {
                        lower = mid;
                    }
                }
                return Some(color);
            }
            previous = color.l;
        }
        None
    };
    if let Some(color) = at_chroma(source.c) {
        return Ok(color);
    }
    // Preserve as much observed chroma as possible when no readable in-gamut
    // lightness exists at the original C. Semantic roles retain their floor.
    let floor = if role.is_some() { 0.035 } else { 0.0 };
    let mut best = at_chroma(floor).ok_or("no readable in-gamut text projection")?;
    let mut upper = source.c;
    for _ in 0..24 {
        let c = 0.5 * (best.c + upper);
        if let Some(color) = at_chroma(c) {
            best = color;
        } else {
            upper = c;
        }
    }
    Ok(best)
}

// Match chromoxide-image's private conversion for export-aware projection.
// Integration audits independently decode the exported PPM with that pipeline.
fn displayed_lab(color: Oklch) -> chromoxide::Oklab {
    let (r, g, b) = crate::filter::srgb_u8(color);
    let linear = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(r), linear(g), linear(b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    chromoxide::Oklab {
        l: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn background() -> Oklch {
        Oklch {
            l: 0.12,
            c: 0.01,
            h: 3.0,
        }
    }

    #[test]
    fn staged_text_lifts_a_dark_source_without_changing_its_hue_or_chroma() {
        let source = Oklch {
            l: 0.3,
            c: 0.05,
            h: 3.0,
        };
        let color = project_text(source, background(), None).unwrap();
        assert!(contrast_ratio(luminance(color), luminance(background())) >= MIN_CONTRAST);
        assert!(color.l > source.l);
        assert_eq!(color.c, source.c);
        assert_eq!(color.h, source.h);
        assert!(in_gamut(color));
    }

    #[test]
    fn staged_text_only_reduces_chroma_when_brightness_cannot_make_blue_readable() {
        let source = Oklch {
            l: 0.45,
            c: 0.28,
            h: 265.0_f64.to_radians(),
        };
        let color = project_text(source, background(), None).unwrap();
        assert!(in_gamut(color));
        assert!(contrast_ratio(luminance(color), luminance(background())) >= MIN_CONTRAST);
        assert!(color.c < source.c);
        assert_eq!(color.h, source.h);
    }

    #[test]
    fn staged_text_keeps_an_already_readable_in_gamut_source_exactly() {
        let source = Oklch {
            l: 0.7,
            c: 0.05,
            h: 3.0,
        };
        assert_eq!(project_text(source, background(), None).unwrap(), source);
    }

    #[test]
    fn staged_decoration_keeps_a_missing_dark_mode_without_changing_text_colors() {
        let dark = Oklch {
            l: 0.4,
            c: 0.02,
            h: 3.0,
        };
        let samples = vec![chromoxide::WeightedSample::new(dark.to_oklab(), 1.0, 0.5)];
        let mut colors = vec![
            Oklch {
                l: 0.7,
                c: 0.01,
                h: 3.0
            };
            16
        ];
        colors[0] = background();
        let before = colors.clone();
        reserve_decoration(&mut colors, &samples);
        assert!(colors[4].to_oklab().distance2(dark.to_oklab()) < 1e-20);
        for slot in TEXT_SLOTS {
            assert_eq!(before[slot], colors[slot]);
        }
        assert_eq!(before[0], colors[0]);
    }

    #[test]
    fn staged_decoration_does_not_promote_an_isolated_pixel_to_a_full_slot() {
        let gray = Oklch {
            l: 0.6,
            c: 0.01,
            h: 3.0,
        };
        let red = Oklch {
            l: 0.55,
            c: 0.2,
            h: 0.5,
        };
        let samples = vec![
            chromoxide::WeightedSample::new(gray.to_oklab(), 1.0 - 1.0 / 5120.0, 0.1),
            chromoxide::WeightedSample::new(red.to_oklab(), 1.0 / 5120.0, 1.0),
        ];
        let mut colors = vec![gray; 16];
        reserve_decoration(&mut colors, &samples);
        assert_eq!(colors[4], gray);
    }
}
