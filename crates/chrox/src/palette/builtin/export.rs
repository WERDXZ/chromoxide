use std::collections::HashMap;

use chromoxide::{Oklch, SlotSpec};

pub trait BuiltinExport {
    fn export(&self, slots: &[SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DirectExport;

impl BuiltinExport for DirectExport {
    fn export(&self, slots: &[SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch> {
        let mut out = HashMap::with_capacity(colors.len());
        for (slot, color) in slots.iter().zip(colors.iter().copied()) {
            out.insert(slot.name.clone(), color);
        }
        out
    }
}
