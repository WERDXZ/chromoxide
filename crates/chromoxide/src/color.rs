//! Core color structures and Oklab/OkLCh transforms.

use crate::util::{circular_hue_distance, wrap_hue};

/// Color in Oklab coordinates.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oklab {
    /// Perceptual lightness.
    pub l: f64,
    /// Green-red axis.
    pub a: f64,
    /// Blue-yellow axis.
    pub b: f64,
}

impl Oklab {
    /// Squared Euclidean distance in Oklab.
    pub fn distance2(self, other: Self) -> f64 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        dl * dl + da * da + db * db
    }

    /// Converts to OkLCh.
    pub fn to_oklch(self) -> Oklch {
        Oklch::from_oklab(self)
    }
}

/// Color in OkLCh coordinates.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness.
    pub l: f64,
    /// Chroma.
    pub c: f64,
    /// Hue angle in radians.
    pub h: f64,
}

impl Oklch {
    /// Creates OkLCh from Oklab.
    pub fn from_oklab(lab: Oklab) -> Self {
        let c = (lab.a * lab.a + lab.b * lab.b).sqrt();
        let h = wrap_hue(lab.b.atan2(lab.a));
        Self { l: lab.l, c, h }
    }

    /// Converts to Oklab.
    pub fn to_oklab(self) -> Oklab {
        let h = wrap_hue(self.h);
        Oklab {
            l: self.l,
            a: self.c * h.cos(),
            b: self.c * h.sin(),
        }
    }

    /// Circular hue distance in radians.
    pub fn hue_distance(self, other: Self) -> f64 {
        circular_hue_distance(self.h, other.h)
    }
}
