//! Valid-by-construction decode from latent space to slot colors.

use crate::cap::{CapInterpolation, ImageCap};
use crate::color::{Oklab, Oklch};
use crate::domain::{CapPolicy, SlotDomain};
use crate::error::PaletteError;
use crate::problem::SlotSpec;
use crate::util::sigmoid;

/// Decoded slot with cap metadata.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DecodedSlot {
    /// Color in Oklab.
    pub lab: Oklab,
    /// Color in OkLCh.
    pub lch: Oklch,
    /// Raw queried cap at `(L, h)`.
    pub cap_at_lh: Option<f64>,
    /// Effective cap limit used during decode (if applicable).
    pub effective_cap_limit: Option<f64>,
}

/// Returns latent dimensionality for a set of slots.
pub fn latent_dim(slot_count: usize) -> usize {
    slot_count * 3
}

/// Decodes all slots from a flat latent vector.
///
/// Latent layout is `[u_l0, u_c0, u_h0, u_l1, u_c1, u_h1, ...]`.
pub fn decode_slots(
    latent: &[f64],
    slots: &[SlotSpec],
    image_cap: Option<&ImageCap>,
) -> Result<Vec<DecodedSlot>, PaletteError> {
    decode_slots_with_interpolation(latent, slots, image_cap, CapInterpolation::default())
}

/// Decodes all slots from a flat latent vector with explicit cap interpolation mode.
pub fn decode_slots_with_interpolation(
    latent: &[f64],
    slots: &[SlotSpec],
    image_cap: Option<&ImageCap>,
    cap_interpolation: CapInterpolation,
) -> Result<Vec<DecodedSlot>, PaletteError> {
    let expected = latent_dim(slots.len());
    if latent.len() != expected {
        return Err(PaletteError::InvalidProblem(format!(
            "latent length mismatch: expected {expected}, got {}",
            latent.len()
        )));
    }

    let mut out = Vec::with_capacity(slots.len());
    for (i, slot) in slots.iter().enumerate() {
        let base = i * 3;
        out.push(decode_slot_with_interpolation(
            &slot.domain,
            latent[base],
            latent[base + 1],
            latent[base + 2],
            image_cap,
            cap_interpolation,
        )?);
    }
    Ok(out)
}

/// Decodes a single slot.
///
/// This uses default cap interpolation when cap lookup is required.
pub fn decode_slot(
    domain: &SlotDomain,
    u_l: f64,
    u_c: f64,
    u_h: f64,
    image_cap: Option<&ImageCap>,
) -> Result<DecodedSlot, PaletteError> {
    decode_slot_with_interpolation(
        domain,
        u_l,
        u_c,
        u_h,
        image_cap,
        CapInterpolation::default(),
    )
}

/// Decodes a single slot with explicit cap interpolation mode.
///
/// Decoding rules:
/// - `L = l_min + sigmoid(u_l) * (l_max - l_min)`
/// - `h` decoded by `HueDomain` (`Any` wraps, `Arc` maps into arc)
/// - `C` decoded from `u_c` into either user chroma bounds or hard-intersected bounds
///
/// In `HardIntersect` mode this function guarantees `C <= queried_cap` by construction.
pub fn decode_slot_with_interpolation(
    domain: &SlotDomain,
    u_l: f64,
    u_c: f64,
    u_h: f64,
    image_cap: Option<&ImageCap>,
    cap_interpolation: CapInterpolation,
) -> Result<DecodedSlot, PaletteError> {
    let l = domain.lightness.decode(u_l);
    let h = domain.hue.decode(u_h);

    let user_c_min = domain.chroma.min;
    let user_c_max = domain.chroma.max;

    let cap_at_lh = image_cap.map(|cap| cap.query_with(l, h, cap_interpolation));

    let (c_min, c_max, effective_cap_limit) = match domain.cap_policy {
        CapPolicy::Ignore => (user_c_min, user_c_max, None),
        CapPolicy::HardIntersect => {
            let cap = cap_at_lh.ok_or_else(|| {
                PaletteError::InvalidProblem(
                    "HardIntersect requires problem.image_cap to be present".to_string(),
                )
            })?;
            let limit = user_c_max.min(cap);
            let c_min_eff = user_c_min.min(limit);
            (c_min_eff, limit, Some(limit))
        }
        CapPolicy::SoftPenalty { .. } => (user_c_min, user_c_max, cap_at_lh),
    };

    let c_span = (c_max - c_min).max(0.0);
    let c = c_min + sigmoid(u_c) * c_span;

    let lch = Oklch { l, c, h };
    Ok(DecodedSlot {
        lab: lch.to_oklab(),
        lch,
        cap_at_lh,
        effective_cap_limit,
    })
}
