//! CODEX (GPT-6): shared post-it typography controls for the lobby and standalone lab.
use bevy::prelude::*;
use crate::{post_it_fonts::{FontChoice, FONTS}, post_it_text};

// CODEX: row order starts at top-left; the default selection is middle-middle (index four).
pub(crate) const ALIGNMENTS: [&str; 9] = ["top-left", "top-middle", "top-right", "middle-left", "middle-middle", "middle-right", "bottom-left", "bottom-middle", "bottom-right"];
// CODEX: small, visible steps retain the lab's tuning feel in the lobby.
const PADDING_STEP: f32 = 0.02;
const FONT_SIZE_STEP: f32 = 4.0;

// CODEX: each note retains its own typography after finishing; subsequent notes start from defaults.
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct PaperStyle {
    pub font_size: f32,
    pub font_index: usize,
    pub alignment: usize,
    pub margin: f32,
}

// CODEX: preserve Gelasio, centered text, and the lab's largest-fitting initial size.
impl Default for PaperStyle {
    // CODEX: reflected scenes and freshly taken notes use the same defaults as the lab.
    fn default() -> Self {
        Self { font_size: 1024.0, font_index: 0, alignment: 4, margin: 0.12 }
    }
}

// CODEX: all control behavior and rendering choices are shared, without owning note lifecycle actions.
impl PaperStyle {
    // CODEX: tolerate reflected dev-console values outside the catalog's range.
    pub fn font(&self) -> &'static FontChoice { &FONTS[self.font_index % FONTS.len()] }

    // CODEX: ordinary arrows resize/cycle fonts; modifiers adjust wrapping, padding and placement.
    pub fn adjust(&mut self, text: &str, size: u32, key: KeyCode, shift: bool, ctrl: bool) -> bool {
        let maximum_cap = size.clamp(64, 1024) as f32 * 2.0;
        match key {
            KeyCode::ArrowUp | KeyCode::ArrowDown if ctrl => {
                self.font_size = post_it_text::font_size_for_line_change(text, size, self.margin,
                    self.font_size, maximum_cap, self.font().bytes, key == KeyCode::ArrowUp);
            }
            KeyCode::ArrowUp if shift => self.margin = (self.margin + PADDING_STEP).min(0.4),
            KeyCode::ArrowDown if shift => self.margin = (self.margin - PADDING_STEP).max(0.02),
            KeyCode::ArrowUp | KeyCode::ArrowDown => {
                let current = post_it_text::fitted_font_size(text, size, self.margin, self.font_size, self.font().bytes);
                let maximum = post_it_text::fitted_font_size(text, size, self.margin, maximum_cap, self.font().bytes);
                let delta = if key == KeyCode::ArrowUp { FONT_SIZE_STEP } else { -FONT_SIZE_STEP };
                self.font_size = (current + delta).clamp(1.0, maximum.max(1.0));
            }
            KeyCode::ArrowLeft if shift => self.alignment = (self.alignment + ALIGNMENTS.len() - 1) % ALIGNMENTS.len(),
            KeyCode::ArrowRight if shift => self.alignment = (self.alignment + 1) % ALIGNMENTS.len(),
            KeyCode::ArrowLeft => self.font_index = (self.font_index + FONTS.len() - 1) % FONTS.len(),
            KeyCode::ArrowRight => self.font_index = (self.font_index + 1) % FONTS.len(),
            _ => return false,
        }
        true
    }

    // CODEX: the same style produces identical pixels in the lab and on the lobby's curled mesh.
    pub fn image(&self, text: &str, editing: bool, cursor: bool, size: u32) -> Image {
        let position = self.alignment.min(8);
        post_it_text::rasterize_aligned(text, cursor, editing, size, self.margin, self.font_size,
            self.font().bytes, UVec2::new((position % 3) as u32, (position / 3) as u32))
    }

    // CODEX: cache this description when text/style changes, avoiding layout work every HUD frame.
    pub fn description(&self, text: &str, size: u32) -> String {
        let px = post_it_text::fitted_font_size(text, size, self.margin, self.font_size, self.font().bytes);
        let lines = post_it_text::fitted_line_count(text, size, self.margin, self.font_size, self.font().bytes);
        format!("{} ({}) | {:.0}% padding | {:.1}px / {} lines | {}",
            self.font().name, self.font().style, self.margin * 100.0, px, lines, ALIGNMENTS[self.alignment.min(8)])
    }
}
