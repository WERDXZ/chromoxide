use std::collections::HashMap;

use chromoxide::{Oklch, SlotSpec};

pub trait BuiltinExport {
    fn members(&self, slots: &[SlotSpec]) -> Vec<String>;
    fn export(&self, slots: &[SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DirectExport;

impl BuiltinExport for DirectExport {
    fn members(&self, slots: &[SlotSpec]) -> Vec<String> {
        slots.iter().map(|slot| slot.name.clone()).collect()
    }

    fn export(&self, slots: &[SlotSpec], colors: &[Oklch]) -> HashMap<String, Oklch> {
        let mut out = HashMap::with_capacity(colors.len());
        for (slot, color) in slots.iter().zip(colors.iter().copied()) {
            out.insert(slot.name.clone(), color);
        }
        out
    }
}
