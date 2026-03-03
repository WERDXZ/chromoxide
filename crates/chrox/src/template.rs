//! Templating engine.
//!
//! Simple syntax: {{palette.name (| filter)?}}
//! Supports simple replace feature and maybe color format conversion.

/// A template with placeholders.
#[derive(Debug)]
pub struct Template {
    source: String,
}

impl Template {
    /// Parse a template from a string.
    pub fn new(source: String) -> Self {
        Self { source }
    }

    /// Render the template using the given palette.
    pub fn render(&self, _palette: &()) -> String {
        // TODO: implement actual substitution
        // For now, just return the source
        self.source.clone()
    }
}

// TODO: define filters (e.g., hex, rgb, hsl)
