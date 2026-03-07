use chromoxide::{
    CapPolicy, ChromaTargetTerm, GroupAxis, GroupMember, GroupQuantileTerm, GroupTarget, HueDomain,
    Interval, Monotonicity, ScalarTarget, SlotDomain, SlotSpec, Term, WeightedTerm,
};

pub fn unconstrained_slot(name: &str) -> SlotSpec {
    SlotSpec {
        name: name.into(),
        domain: SlotDomain {
            lightness: Interval { min: 0.0, max: 1.0 },
            chroma: Interval { min: 0.0, max: 1.0 },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::HardIntersect,
            chroma_epsilon: 0.02,
        },
    }
}

pub fn neutral_slot(name: &str, lightness: Interval, chroma_max: f64) -> SlotSpec {
    SlotSpec {
        name: name.into(),
        domain: SlotDomain {
            lightness,
            chroma: Interval {
                min: 0.0,
                max: chroma_max,
            },
            hue: HueDomain::Any,
            cap_policy: CapPolicy::HardIntersect,
            chroma_epsilon: 0.02,
        },
    }
}

pub fn accent_slot(
    name: &str,
    start_deg: f64,
    len_deg: f64,
    lightness: Interval,
    chroma: Interval,
) -> SlotSpec {
    SlotSpec {
        name: name.into(),
        domain: SlotDomain {
            lightness,
            chroma,
            hue: HueDomain::Arc {
                start: deg(start_deg),
                len: deg(len_deg),
            },
            cap_policy: CapPolicy::HardIntersect,
            chroma_epsilon: 0.02,
        },
    }
}

pub fn weighted(name: &str, weight: f64, term: Term) -> WeightedTerm {
    WeightedTerm {
        weight,
        name: Some(name.into()),
        term,
    }
}

pub fn neutral_ladder_term(
    name: &str,
    slots: &[usize],
    values: Vec<f64>,
    monotonic: Monotonicity,
) -> WeightedTerm {
    weighted(
        name,
        8.0,
        Term::GroupQuantile(GroupQuantileTerm {
            members: slots
                .iter()
                .copied()
                .map(|slot| GroupMember { slot, mass: 1.0 })
                .collect(),
            axis: GroupAxis::Lightness,
            target: GroupTarget::ExplicitValues(values),
            monotonic: Some(monotonic),
            huber_delta: 0.02,
        }),
    )
}

pub fn low_chroma_term(name: &str, slot: usize, max: f64) -> WeightedTerm {
    weighted(
        name,
        2.0,
        Term::ChromaTarget(ChromaTargetTerm {
            slot,
            target: ScalarTarget::Max(max),
            hinge_delta: Some(0.02),
        }),
    )
}

pub fn deg(value: f64) -> f64 {
    value.to_radians()
}
