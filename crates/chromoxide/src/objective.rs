//! Objective evaluation and finite-difference gradients.

use crate::convert::{oklab_to_linear_srgb, relative_luminance};
use crate::decode::DecodedSlot;
use crate::diagnostics::TermBreakdown;
use crate::domain::CapPolicy;
use crate::error::PaletteError;
use crate::problem::PaletteProblem;
use crate::term::EvalContext;
use crate::terms::saliency::estimate_saliency_components_at;
use crate::util::{l2_norm, pseudo_huber, relu, smoothstep01};

/// Cached decoded palette and precomputed fields.
#[derive(Clone, Debug)]
pub struct DecodedPalette {
    /// Decoded slots.
    pub slots: Vec<DecodedSlot>,
    /// Relative luminance per slot.
    pub luminance: Vec<f64>,
    /// Hue reliability gate per slot.
    pub hue_gates: Vec<f64>,
    /// Estimated saliency per slot.
    pub estimated_saliency: Vec<f64>,
}

/// Objective evaluator for a palette problem.
#[derive(Clone, Debug)]
pub struct ObjectiveEvaluator<'a> {
    /// Problem reference.
    pub problem: &'a PaletteProblem,
}

impl<'a> ObjectiveEvaluator<'a> {
    /// Creates a new evaluator.
    pub fn new(problem: &'a PaletteProblem) -> Result<Self, PaletteError> {
        Ok(Self { problem })
    }

    /// Decodes latent vector and computes precomputed fields.
    ///
    /// Precomputed fields include relative luminance, hue reliability gates,
    /// and optional saliency estimates used in diagnostics.
    pub fn decode_palette(&self, latent: &[f64]) -> Result<DecodedPalette, PaletteError> {
        let decoded = crate::decode::decode_slots_with_interpolation(
            latent,
            &self.problem.slots,
            self.problem.image_cap.as_ref(),
            self.problem.config.cap_interpolation,
        )?;

        let mut luminance = Vec::with_capacity(decoded.len());
        let mut hue_gates = Vec::with_capacity(decoded.len());
        let mut estimated_saliency = Vec::with_capacity(decoded.len());

        let saliency_params = self.infer_saliency_params();
        for (i, slot) in decoded.iter().enumerate() {
            let y = relative_luminance(oklab_to_linear_srgb(slot.lab));
            luminance.push(y);

            let eps = self.problem.slots[i].domain.chroma_epsilon;
            let gate = if eps <= 0.0 {
                1.0
            } else {
                smoothstep01(slot.lch.c / eps)
            };
            hue_gates.push(gate);

            let params = saliency_params[i];
            let est = estimate_saliency_components_at(
                slot.lab,
                &self.problem.samples,
                params.sigma,
                params.support_scale,
            );
            estimated_saliency.push(est.effective);
        }

        Ok(DecodedPalette {
            slots: decoded,
            luminance,
            hue_gates,
            estimated_saliency,
        })
    }

    /// Evaluates objective and returns breakdown with decoded palette.
    ///
    /// This includes explicit weighted term contributions and soft-cap
    /// penalties for strict and adaptive soft-cap slots.
    pub fn evaluate_breakdown(
        &self,
        latent: &[f64],
    ) -> Result<(f64, Vec<TermBreakdown>, DecodedPalette), PaletteError> {
        let decoded = self.decode_palette(latent)?;

        let labs: Vec<_> = decoded.slots.iter().map(|s| s.lab).collect();
        let lchs: Vec<_> = decoded.slots.iter().map(|s| s.lch).collect();
        let chroma_lower_bounds: Vec<_> = decoded
            .slots
            .iter()
            .map(|s| s.effective_chroma_min)
            .collect();
        let chroma_upper_bounds: Vec<_> = decoded
            .slots
            .iter()
            .map(|s| s.effective_chroma_max)
            .collect();
        let user_chroma_lower_bounds: Vec<_> = self
            .problem
            .slots
            .iter()
            .map(|s| s.domain.chroma.min)
            .collect();
        let user_chroma_upper_bounds: Vec<_> = self
            .problem
            .slots
            .iter()
            .map(|s| s.domain.chroma.max)
            .collect();
        let effective_chroma_lower_bounds = chroma_lower_bounds.clone();
        let effective_chroma_upper_bounds = chroma_upper_bounds.clone();
        let image_cap_chroma_upper_bounds: Vec<Option<f64>> = decoded
            .slots
            .iter()
            .map(|s| {
                self.problem.image_cap.as_ref().map(|cap| {
                    cap.query_with(s.lch.l, s.lch.h, self.problem.config.cap_interpolation)
                })
            })
            .collect();
        let adaptive_image_cap_chroma_upper_bounds: Vec<Option<f64>> = decoded
            .slots
            .iter()
            .map(|s| {
                self.problem.image_cap.as_ref().map(|cap| {
                    cap.query_adaptive_with(s.lch.l, s.lch.h, self.problem.config.cap_interpolation)
                        .chroma
                })
            })
            .collect();
        let ctx = EvalContext {
            slots_lab: &labs,
            slots_lch: &lchs,
            luminance: &decoded.luminance,
            hue_gates: &decoded.hue_gates,
            chroma_lower_bounds: &chroma_lower_bounds,
            chroma_upper_bounds: &chroma_upper_bounds,
            user_chroma_lower_bounds: &user_chroma_lower_bounds,
            user_chroma_upper_bounds: &user_chroma_upper_bounds,
            effective_chroma_lower_bounds: &effective_chroma_lower_bounds,
            effective_chroma_upper_bounds: &effective_chroma_upper_bounds,
            image_cap_chroma_upper_bounds: &image_cap_chroma_upper_bounds,
            adaptive_image_cap_chroma_upper_bounds: &adaptive_image_cap_chroma_upper_bounds,
            samples: &self.problem.samples,
        };

        let mut total = 0.0;
        let mut breakdown = Vec::with_capacity(self.problem.terms.len());

        for wt in &self.problem.terms {
            let eval = wt.term.evaluate(&ctx);
            let weighted = eval.raw * wt.weight;
            if !weighted.is_finite() {
                return Err(PaletteError::NumericInstability(
                    "non-finite weighted term value".to_string(),
                ));
            }
            total += weighted;
            breakdown.push(TermBreakdown {
                name: wt
                    .name
                    .clone()
                    .unwrap_or_else(|| wt.term.default_name().to_string()),
                raw: eval.raw,
                weighted,
                components: eval.components,
            });
        }

        for (i, slot_spec) in self.problem.slots.iter().enumerate() {
            match slot_spec.domain.cap_policy {
                CapPolicy::SoftPenalty {
                    weight,
                    huber_delta,
                } => {
                    let cap = self
                        .problem
                        .image_cap
                        .as_ref()
                        .ok_or_else(|| {
                            PaletteError::InvalidProblem(
                                "SoftPenalty requires problem.image_cap to be present".to_string(),
                            )
                        })?
                        .query_with(
                            decoded.slots[i].lch.l,
                            decoded.slots[i].lch.h,
                            self.problem.config.cap_interpolation,
                        );
                    let overflow = relu(decoded.slots[i].lch.c - cap);
                    let raw = pseudo_huber(overflow, huber_delta);
                    let weighted = raw * weight;
                    if !weighted.is_finite() {
                        return Err(PaletteError::NumericInstability(
                            "non-finite soft cap value".to_string(),
                        ));
                    }
                    total += weighted;
                    breakdown.push(TermBreakdown {
                        name: format!("soft_cap/{}", slot_spec.name),
                        raw,
                        weighted,
                        components: vec![overflow, cap],
                    });
                }
                CapPolicy::AdaptiveSoftPenalty {
                    weight,
                    huber_delta,
                } => {
                    let query = self
                        .problem
                        .image_cap
                        .as_ref()
                        .ok_or_else(|| {
                            PaletteError::InvalidProblem(
                                "AdaptiveSoftPenalty requires problem.image_cap to be present"
                                    .to_string(),
                            )
                        })?
                        .query_adaptive_with(
                            decoded.slots[i].lch.l,
                            decoded.slots[i].lch.h,
                            self.problem.config.cap_interpolation,
                        );
                    let overflow = relu(decoded.slots[i].lch.c - query.chroma);
                    let raw = pseudo_huber(overflow, huber_delta);
                    let weighted = raw * weight;
                    if !weighted.is_finite() {
                        return Err(PaletteError::NumericInstability(
                            "non-finite adaptive soft cap value".to_string(),
                        ));
                    }
                    total += weighted;
                    breakdown.push(TermBreakdown {
                        name: format!("adaptive_soft_cap/{}", slot_spec.name),
                        raw,
                        weighted,
                        components: vec![
                            overflow,
                            query.chroma,
                            query.conditional_chroma,
                            query.global_chroma,
                            query.support_confidence,
                        ],
                    });
                }
                CapPolicy::Ignore | CapPolicy::HardIntersect => {}
            }
        }

        if !total.is_finite() {
            return Err(PaletteError::NumericInstability(
                "objective became non-finite".to_string(),
            ));
        }

        Ok((total, breakdown, decoded))
    }

    /// Evaluates objective scalar.
    pub fn evaluate_total(&self, latent: &[f64]) -> Result<f64, PaletteError> {
        let (total, _, _) = self.evaluate_breakdown(latent)?;
        Ok(total)
    }

    /// Central finite-difference gradient.
    ///
    /// Per-dimension step size is `fd_epsilon * max(1, |u_j|)`.
    /// This is the dominant runtime bottleneck for large sample sets.
    pub fn finite_difference_gradient(
        &self,
        latent: &[f64],
        fd_epsilon: f64,
    ) -> Result<Vec<f64>, PaletteError> {
        let mut grad = vec![0.0; latent.len()];
        for j in 0..latent.len() {
            let eps_j = fd_epsilon * latent[j].abs().max(1.0);

            let mut plus = latent.to_vec();
            plus[j] += eps_j;
            let e_plus = self.evaluate_total(&plus)?;

            let mut minus = latent.to_vec();
            minus[j] -= eps_j;
            let e_minus = self.evaluate_total(&minus)?;

            let g = (e_plus - e_minus) / (2.0 * eps_j);
            if !g.is_finite() {
                return Err(PaletteError::NumericInstability(format!(
                    "non-finite gradient at dim {j}"
                )));
            }
            grad[j] = g;
        }
        Ok(grad)
    }

    /// Computes gradient norm at latent point.
    pub fn gradient_norm(&self, latent: &[f64], fd_epsilon: f64) -> Result<f64, PaletteError> {
        let g = self.finite_difference_gradient(latent, fd_epsilon)?;
        Ok(l2_norm(&g))
    }

    /// Infers per-slot saliency kernel parameters from configured saliency terms.
    fn infer_saliency_params(&self) -> Vec<SaliencyParams> {
        let mut params = vec![
            SaliencyParams {
                sigma: 0.08,
                support_scale: 0.05,
            };
            self.problem.slots.len()
        ];
        for wt in &self.problem.terms {
            if let crate::term::Term::Saliency(t) = &wt.term {
                params[t.slot] = SaliencyParams {
                    sigma: t.sigma.max(1.0e-6),
                    support_scale: t.support_scale.max(1.0e-12),
                };
            }
        }
        params
    }
}

#[derive(Clone, Copy, Debug)]
struct SaliencyParams {
    sigma: f64,
    support_scale: f64,
}
