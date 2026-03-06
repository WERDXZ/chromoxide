use std::collections::HashMap;

use chromoxide::{Oklch, SlotSpec};

pub trait BuiltinExport {
    fn export(&self, slots: &[SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch>;
}
