use chromoxide::{
    CapPolicy, HueDomain, ImageCapBuilder, Interval, Oklch, SlotDomain, WeightedSample,
    decode::decode_slot,
};

#[test]
fn decode_stays_inside_slot_domain() {
    let domain = SlotDomain {
        lightness: Interval { min: 0.2, max: 0.8 },
        chroma: Interval {
            min: 0.01,
            max: 0.18,
        },
        hue: HueDomain::Arc {
            start: 0.6,
            len: 1.2,
        },
        cap_policy: CapPolicy::Ignore,
        chroma_epsilon: 0.03,
    };

    for i in -20..=20 {
        let u = i as f64;
        let decoded = decode_slot(&domain, 0.7 * u, -0.4 * u, 1.3 * u, None).unwrap();
        assert!(decoded.lch.l >= domain.lightness.min - 1.0e-12);
        assert!(decoded.lch.l <= domain.lightness.max + 1.0e-12);
        assert!(decoded.lch.c >= domain.chroma.min - 1.0e-12);
        assert!(decoded.lch.c <= domain.chroma.max + 1.0e-12);
        assert!(domain.hue.contains(decoded.lch.h));
    }
}

#[test]
fn hard_cap_decode_never_exceeds_cap() {
    let samples = vec![
        WeightedSample::new(
            Oklch {
                l: 0.5,
                c: 0.04,
                h: 1.0,
            }
            .to_oklab(),
            10.0,
            1.0,
        ),
        WeightedSample::new(
            Oklch {
                l: 0.52,
                c: 0.045,
                h: 1.1,
            }
            .to_oklab(),
            8.0,
            0.8,
        ),
    ];

    let cap = ImageCapBuilder {
        n_l: 8,
        n_h: 24,
        smooth_l_radius: 0,
        smooth_h_radius: 0,
        relax: 1.0,
    }
    .build(&samples)
    .unwrap();

    let domain = SlotDomain {
        lightness: Interval { min: 0.3, max: 0.8 },
        chroma: Interval { min: 0.0, max: 0.2 },
        hue: HueDomain::Any,
        cap_policy: CapPolicy::HardIntersect,
        chroma_epsilon: 0.02,
    };

    for i in -5..=5 {
        let decoded = decode_slot(&domain, i as f64, 8.0, i as f64 * 0.4, Some(&cap)).unwrap();
        let queried_cap = cap.query(decoded.lch.l, decoded.lch.h);
        assert!(decoded.lch.c <= queried_cap + 1.0e-10);
    }
}

#[test]
fn hue_arc_cross_zero_uses_explicit_len_without_ambiguity() {
    let domain = SlotDomain {
        lightness: Interval { min: 0.4, max: 0.6 },
        chroma: Interval {
            min: 0.01,
            max: 0.12,
        },
        hue: HueDomain::Arc {
            start: 5.9,
            len: 1.0,
        },
        cap_policy: CapPolicy::Ignore,
        chroma_epsilon: 0.02,
    };

    let lo = decode_slot(&domain, 0.0, 0.0, -8.0, None).unwrap();
    let hi = decode_slot(&domain, 0.0, 0.0, 8.0, None).unwrap();
    assert!(domain.hue.contains(lo.lch.h));
    assert!(domain.hue.contains(hi.lch.h));

    let expected_end = chromoxide::util::wrap_hue(5.9 + 1.0);
    assert!(
        (hi.lch.h - expected_end).abs() < 0.05
            || (hi.lch.h + std::f64::consts::TAU - expected_end).abs() < 0.05
    );
}
