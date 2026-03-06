use chromoxide::{CapPolicy, HueDomain, Interval, SlotDomain, SlotSpec};

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
