//! CODEX (GPT-6): shared font catalog for the lobby and standalone lab; Gelasio stays the default.
//! Upstream font files and licenses are recorded in assets/fonts/post_it/CODEX_FONT_SOURCES.json.

// CODEX: font bytes are embedded so the standalone binary also works offline and from any directory.
pub(crate) struct FontChoice {
    pub name: &'static str,
    pub style: &'static str,
    pub bytes: &'static [u8],
}

// CODEX: preserve the default first, a standard sans second, and the requested cursive face third.
pub(crate) const FONTS: &[FontChoice] = &[
    FontChoice { name: "Gelasio Italic", style: "default serif", bytes: super::post_it_text::FONT_BYTES },
    FontChoice { name: "Roboto", style: "clean sans", bytes: include_bytes!("../assets/fonts/roboto.ttf") },
    FontChoice { name: "Great Vibes", style: "flowing cursive", bytes: include_bytes!("../assets/fonts/post_it/GreatVibes-Regular.ttf") },
    FontChoice { name: "Space Mono", style: "monospace", bytes: include_bytes!("../assets/fonts/post_it/SpaceMono-Regular.ttf") },
    FontChoice { name: "Bangers", style: "comic lettering", bytes: include_bytes!("../assets/fonts/post_it/Bangers-Regular.ttf") },
    FontChoice { name: "Fredericka the Great", style: "sketched serif", bytes: include_bytes!("../assets/fonts/post_it/FrederickatheGreat-Regular.ttf") },
    FontChoice { name: "Monoton", style: "striped retro display", bytes: include_bytes!("../assets/fonts/post_it/Monoton-Regular.ttf") },
    FontChoice { name: "Press Start 2P", style: "pixel arcade", bytes: include_bytes!("../assets/fonts/post_it/PressStart2P-Regular.ttf") },
    FontChoice { name: "Special Elite", style: "worn typewriter", bytes: include_bytes!("../assets/fonts/post_it/SpecialElite-Regular.ttf") },
    FontChoice { name: "Amatic SC", style: "tall hand lettering", bytes: include_bytes!("../assets/fonts/post_it/AmaticSC-Bold.ttf") },
    FontChoice { name: "Unifraktur Maguntia", style: "blackletter", bytes: include_bytes!("../assets/fonts/post_it/UnifrakturMaguntia-Book.ttf") },
    FontChoice { name: "Rye", style: "western poster", bytes: include_bytes!("../assets/fonts/post_it/Rye-Regular.ttf") },
];

// CODEX: distinct fonts must all render readable ink inside the same centered writing area.
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    // CODEX: exercise the renderer with every real asset; decorative swashes can exceed font advances.
    #[test]
    fn every_font_fits_centered_with_padding() {
        for choice in FONTS {
            for text in ["Corn", "A little reminder\nto bring the corn home."] {
                let image = super::super::post_it_text::rasterize_with_font(text, false, true, 512, 0.12, 1024.0, choice.bytes);
                let mut bounds = Rect::EMPTY;
                for (i, pixel) in image.data.as_ref().unwrap().chunks_exact(4).enumerate() {
                    if pixel[3] > 8 { bounds = bounds.union_point(Vec2::new((i % 512) as f32, (i / 512) as f32)); }
                }
                assert!(bounds.width() > 0.0, "{}: no ink", choice.name);
                assert!((bounds.center() - Vec2::splat(256.0)).abs().max_element() <= 3.0, "{}: {bounds:?}", choice.name);
                assert!(bounds.min.min_element() >= 59.0 && bounds.max.max_element() <= 453.0, "{}: outside padding: {bounds:?}", choice.name);
            }
        }
    }

    // CODEX: wrapping thresholds vary by typeface; Ctrl-size jumps must work with every selectable face.
    #[test]
    fn line_count_jumps_work_with_every_font() {
        use super::super::post_it_text::{fitted_line_count, font_size_for_line_change};
        let text = "one two three four five six seven eight nine ten";
        for choice in FONTS {
            let count = fitted_line_count(text, 512, 0.12, 1024.0, choice.bytes);
            let smaller = font_size_for_line_change(text, 512, 0.12, 1024.0, 1024.0, choice.bytes, false);
            let fewer = fitted_line_count(text, 512, 0.12, smaller, choice.bytes);
            assert!(fewer < count, "{}: Ctrl+Down did not remove a line", choice.name);
            let larger = font_size_for_line_change(text, 512, 0.12, smaller, 1024.0, choice.bytes, true);
            assert!(fitted_line_count(text, 512, 0.12, larger, choice.bytes) > fewer,
                "{}: Ctrl+Up did not add a line", choice.name);
        }
    }
}
