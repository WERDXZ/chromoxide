use approx::assert_relative_eq;
use chromoxide::{Oklab, Oklch};

#[test]
fn oklab_oklch_roundtrip_is_stable() {
    let samples = [
        Oklab {
            l: 0.2,
            a: 0.03,
            b: -0.02,
        },
        Oklab {
            l: 0.55,
            a: -0.1,
            b: 0.18,
        },
        Oklab {
            l: 0.91,
            a: 0.24,
            b: -0.11,
        },
        Oklab {
            l: 0.42,
            a: 0.0,
            b: 0.0,
        },
    ];

    for lab in samples {
        let lch = Oklch::from_oklab(lab);
        let roundtrip = lch.to_oklab();
        assert_relative_eq!(lab.l, roundtrip.l, epsilon = 1.0e-12);
        assert_relative_eq!(lab.a, roundtrip.a, epsilon = 1.0e-12);
        assert_relative_eq!(lab.b, roundtrip.b, epsilon = 1.0e-12);
    }
}
