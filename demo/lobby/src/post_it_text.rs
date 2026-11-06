//! CODEX (GPT-6): isolated CPU text rasterizer and conforming ink mesh for authored XZ paper.
//! Ink uses a transparent texture on a copy of the paper, following its curled edges without cameras.
use ab_glyph::{point, Font as _, FontRef, PxScale, ScaleFont};
use bevy::{asset::RenderAssetUsages, mesh::VertexAttributeValues, prelude::*, render::render_resource::{Extent3d, TextureDimension, TextureFormat}};

// CODEX: editable text data is shared by the game and the standalone post_it_lab binary.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct PaperText {
    pub text: String,
    pub editing: bool,
    pub action: String,
}

// CODEX: one keyboard edit, independent of the lobby's input ownership and note lifecycle.
pub(crate) fn edit_key(note: &mut PaperText, key: &bevy::input::keyboard::KeyboardInput, max_chars: usize, ctrl: bool) {
    if !note.editing || !note.action.is_empty() || key.state != bevy::input::ButtonState::Pressed { return; }
    match key.key_code {
        KeyCode::Enter | KeyCode::NumpadEnter if ctrl => {
            if note.text.chars().count() < max_chars.clamp(1, 1000) { note.text.push('\n'); }
        }
        KeyCode::Enter | KeyCode::NumpadEnter => note.action = "finish".into(),
        KeyCode::Delete => note.action = "discard".into(),
        KeyCode::Backspace => { note.text.pop(); }
        _ => if let Some(text) = &key.text {
            let remaining = max_chars.clamp(1, 1000).saturating_sub(note.text.chars().count());
            note.text.extend(text.chars().filter(|c| !c.is_control()).take(remaining));
        },
    }
}

// CODEX: use the same bundled italic face as the interaction teletype; works offline and on Wasm.
pub(crate) const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Gelasio-Italic.ttf");

// CODEX: keep explicit line breaks, balancing each paragraph independently at spaces.
fn lines<F: ab_glyph::Font>(text: &str, font: &impl ScaleFont<F>, width: f32) -> Vec<String> {
    text.split('\n').flat_map(|paragraph| balanced_lines(paragraph, font, width)).collect()
}

// CODEX: prefer the fewest lines, then balance spare width. Words remain indivisible even when
// wider than the note at this candidate size; the fitter shrinks the font until they fit whole.
fn balanced_lines<F: ab_glyph::Font>(paragraph: &str, font: &impl ScaleFont<F>, width: f32) -> Vec<String> {
    let words: Vec<_> = paragraph.split_whitespace().collect();
    let count = words.len();
    if count == 0 { return vec![String::new()]; }
    let mut rows = vec![usize::MAX; count + 1];
    let mut cost = vec![f32::INFINITY; count + 1];
    let mut next = vec![count; count];
    rows[count] = 0;
    cost[count] = 0.0;
    for start in (0..count).rev() {
        let mut line = String::new();
        for end in start..count {
            if end > start { line.push(' '); }
            line.push_str(&words[end]);
            // CODEX: wrap using actual ink, including italic overhangs, so reducing font size cannot
            // unexpectedly fail a width check and jump to a much smaller fitted size.
            let used = line_bounds(&line, font.font(), font.scale().y).width();
            if used > width && end > start { break; }
            let candidate_rows = 1 + rows[end + 1];
            let candidate_cost = (width - used).powi(2) + cost[end + 1];
            if candidate_rows < rows[start] || (candidate_rows == rows[start] && candidate_cost < cost[start]) {
                rows[start] = candidate_rows;
                cost[start] = candidate_cost;
                next[start] = end + 1;
            }
        }
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < count {
        result.push(words[start..next[start]].join(" "));
        start = next[start];
    }
    result
}

// CODEX: include kerning in test widths so they match the rasterized glyph positions.
#[cfg(test)]
fn measure<F: ab_glyph::Font>(text: &str, font: &impl ScaleFont<F>) -> f32 {
    let mut width = 0.0;
    let mut previous = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        width += previous.map_or(0.0, |p| font.kern(p, id)) + font.h_advance(id);
        previous = Some(id);
    }
    width
}

// CODEX: visible glyph bounds account for italic overhangs and short letters such as "corn".
fn line_bounds<F: ab_glyph::Font>(text: &str, font: &F, px: f32) -> Rect {
    let scaled = font.as_scaled(PxScale::from(px));
    let mut bounds = Rect::EMPTY;
    let mut x = 0.0;
    let mut previous = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        x += previous.map_or(0.0, |p| scaled.kern(p, id));
        if let Some(glyph) = font.outline_glyph(id.with_scale_and_position(px, point(x, 0.0))) {
            let ink = glyph.px_bounds();
            bounds = bounds.union(Rect::from_corners(Vec2::new(ink.min.x, ink.min.y), Vec2::new(ink.max.x, ink.max.y)));
        }
        x += scaled.h_advance(id);
        previous = Some(id);
    }
    if bounds == Rect::EMPTY { Rect::from_corners(Vec2::ZERO, Vec2::ZERO) } else { bounds }
}

// CODEX: retain measured rows so both fitting and drawing use the same visible ink rectangle.
struct NoteLayout {
    px: f32,
    rows: Vec<(String, Rect)>,
    top: f32,
    bottom: f32,
    width: f32,
}

// CODEX: measure each centered row at a candidate size, including its actual vertical ink extent.
fn at_size(text: &str, font: &FontRef<'_>, px: f32, available: f32) -> NoteLayout {
    let scaled = font.as_scaled(PxScale::from(px));
    let step = scaled.height() + scaled.line_gap();
    let rows: Vec<_> = lines(text, &scaled, available).into_iter().map(|line| {
        // CODEX: blank lines reserve a full line of height, including leading/trailing manual breaks.
        let bounds = if line.is_empty() {
            Rect::from_corners(Vec2::new(0.0, -scaled.ascent()), Vec2::new(0.0, -scaled.descent()))
        } else { line_bounds(&line, font, px) };
        (line, bounds)
    }).collect();
    let mut result = NoteLayout { px, rows, top: f32::MAX, bottom: f32::MIN, width: 0.0 };
    for (i, (_, bounds)) in result.rows.iter().enumerate() {
        result.top = result.top.min(i as f32 * step + bounds.min.y);
        result.bottom = result.bottom.max(i as f32 * step + bounds.max.y);
        result.width = result.width.max(bounds.width());
    }
    result
}

// CODEX: grow to the largest fitting size without ever splitting a word, including long runs.
fn layout(text: &str, size: u32, margin: f32, max_font: f32, font: &FontRef<'_>) -> NoteLayout {
    let available = size as f32 * (1.0 - margin.clamp(0.02, 0.4) * 2.0);
    // CODEX: measure actual glyph bounds at every candidate, including italic overhangs. An
    // advance-width cap can underestimate those bounds and previously trigger a split in "postit".
    let mut high = max_font.clamp(1.0, size as f32 * 2.0);
    let mut low = 0.001;
    let mut best = at_size(text, font, low, available);
    // CODEX: bounded search keeps fitting inexpensive during typing, even for long unbroken messages.
    for _ in 0..14 {
        let px = (low + high) * 0.5;
        let candidate = at_size(text, font, px, available);
        if candidate.width <= available && candidate.bottom - candidate.top <= available {
            low = px;
            best = candidate;
        } else {
            high = px;
        }
    }
    best
}

// CODEX: all pixels outside actual ink remain transparent, preserving the original yellow material.
#[allow(dead_code)] // CODEX: default-face convenience for the lab and raster regression tests.
pub(crate) fn rasterize(text: &str, cursor: bool, editing: bool, size: u32, margin: f32, max_font: f32) -> Image {
    rasterize_with_font(text, cursor, editing, size, margin, max_font, FONT_BYTES)
}

// CODEX: measure only the message. Empty rows reserve full font height, so an empty cursor starts big.
fn fit_text(text: &str, size: u32, margin: f32, max_font: f32, font: &FontRef<'_>) -> NoteLayout {
    layout(text, size.clamp(64, 1024), margin, max_font, font)
}

// CODEX: size controls start at the actual rendered size, even when their configured ceiling is higher.
pub(crate) fn fitted_font_size(text: &str, size: u32, margin: f32, max_font: f32, font_bytes: &[u8]) -> f32 {
    let font = FontRef::try_from_slice(font_bytes).expect("valid bundled post-it font");
    fit_text(text, size, margin, max_font, &font).px
}

// CODEX: report the same wrapped rows the renderer uses, including intentional blank/manual lines.
pub(crate) fn fitted_line_count(text: &str, size: u32, margin: f32, max_font: f32, font_bytes: &[u8]) -> usize {
    let font = FontRef::try_from_slice(font_bytes).expect("valid bundled post-it font");
    fit_text(text, size, margin, max_font, &font).rows.len()
}

// CODEX: jump to the nearest size that changes wrapping, preserving explicit newlines. Search actual
// row counts between the current fit and its bounds; Ctrl+Up at the largest count fills the note.
pub(crate) fn font_size_for_line_change(text: &str, size: u32, margin: f32, current_cap: f32, maximum_cap: f32, font_bytes: &[u8], increase: bool) -> f32 {
    let font = FontRef::try_from_slice(font_bytes).expect("valid bundled post-it font");
    let current = fit_text(text, size, margin, current_cap, &font);
    let maximum = fit_text(text, size, margin, maximum_cap, &font);
    let available = size.clamp(64, 1024) as f32 * (1.0 - 2.0 * margin.clamp(0.02, 0.4));
    let count = current.rows.len();
    if increase && maximum.rows.len() <= count { return maximum.px; }
    if !increase && at_size(text, &font, 1.0, available).rows.len() >= count { return current_cap; }
    let (mut low, mut high) = if increase { (current.px, maximum.px) } else { (1.0, current.px) };
    for _ in 0..18 {
        let mid = (low + high) * 0.5;
        let rows = at_size(text, &font, mid, available).rows.len();
        let crossed = if increase { rows > count } else { rows >= count };
        if crossed { high = mid; } else { low = mid; }
    }
    // CODEX: clear the threshold by more than the fitter's precision so rendering keeps the new count.
    let epsilon = (maximum.px / 16384.0 * 2.0).max(0.02);
    if increase { (high + epsilon).min(maximum.px) } else { (low - epsilon).max(1.0) }
}

// CODEX: the lab selects a face explicitly; the game's wrapper preserves the existing default font.
#[allow(dead_code)] // CODEX: font-specific convenience used by raster regression tests.
pub(crate) fn rasterize_with_font(text: &str, cursor: bool, editing: bool, size: u32, margin: f32, max_font: f32, font_bytes: &[u8]) -> Image {
    rasterize_aligned(text, cursor, editing, size, margin, max_font, font_bytes, UVec2::ONE)
}

// CODEX: column/row 0, 1, 2 select left/center/right and top/middle/bottom within the padded note.
pub(crate) fn rasterize_aligned(text: &str, cursor: bool, editing: bool, size: u32, margin: f32, max_font: f32, font_bytes: &[u8], alignment: UVec2) -> Image {
    let size = size.clamp(64, 1024);
    let font = FontRef::try_from_slice(font_bytes).expect("valid bundled post-it font");
    // CODEX: the cursor never affects font size, wrapping, centering or the Enter-to-finish transition.
    let fitted = fit_text(text, size, margin, max_font, &font);
    let px = fitted.px;
    let scaled = font.as_scaled(PxScale::from(px));
    let inset = size as f32 * margin.clamp(0.02, 0.4);
    let available = size as f32 - 2.0 * inset;
    let align = alignment.min(UVec2::splat(2)).as_vec2() * 0.5;
    let baseline = inset + (available - (fitted.bottom - fitted.top)) * align.y - fitted.top;
    let mut pixels = vec![0u8; size as usize * size as usize * 4];
    let mut caret = Vec2::ZERO;
    for (row, (line, bounds)) in fitted.rows.iter().enumerate() {
        let mut x = inset + (available - bounds.width()) * align.x - bounds.min.x;
        let y = baseline + row as f32 * (scaled.height() + scaled.line_gap());
        let mut previous = None;
        for c in line.chars() {
            let id = scaled.glyph_id(c);
            x += previous.map_or(0.0, |p| scaled.kern(p, id));
            let glyph = id.with_scale_and_position(px, point(x, y));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|dx, dy, coverage| {
                    let x = bounds.min.x as i32 + dx as i32;
                    let y = bounds.min.y as i32 + dy as i32;
                    if x >= 0 && y >= 0 && x < size as i32 && y < size as i32 {
                        let i = (y as usize * size as usize + x as usize) * 4;
                        // CODEX: retain the original dark navy ink on the transparent overlay.
                        pixels[i..i + 3].copy_from_slice(&[4, 7, 15]);
                        pixels[i + 3] = pixels[i + 3].max((coverage * 255.0) as u8);
                    }
                });
            }
            x += scaled.h_advance(id);
            previous = Some(id);
        }
        caret = Vec2::new(x, y);
    }
    if editing && cursor {
        let width = (px * 0.025).max(2.0);
        let height = scaled.height().min(size as f32 - 2.0);
        let spaces = text.chars().rev().take_while(|c| *c == ' ').count() as f32;
        let x = if text.is_empty() { caret.x - width * 0.5 }
            else { caret.x + scaled.h_advance(scaled.glyph_id(' ')) * spaces + px * 0.02 };
        let origin = Vec2::new(x, caret.y - scaled.ascent()).clamp(Vec2::ONE, Vec2::new(size as f32 - width - 1.0, size as f32 - height - 1.0));
        draw_cursor(&mut pixels, size, Rect::from_corners(origin, origin + Vec2::new(width, height)));
    }
    Image::new(Extent3d { width: size, height: size, depth_or_array_layers: 1 }, TextureDimension::D2, pixels,
        TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
}

// CODEX: isolated procedural caret overlay; it occupies no space in the text layout or font bounds.
fn draw_cursor(pixels: &mut [u8], size: u32, bounds: Rect) {
    for y in bounds.min.y.ceil() as u32..bounds.max.y.ceil() as u32 {
        for x in bounds.min.x.ceil() as u32..bounds.max.x.ceil() as u32 {
            let i = (y as usize * size as usize + x as usize) * 4;
            // CODEX: the caret uses the same dark navy as the message.
            pixels[i..i + 4].copy_from_slice(&[4, 7, 15, 255]);
        }
    }
}

// CODEX: remap the source atlas UVs to the note's local XZ writing area; offset along its actual normals.
pub(crate) fn ink_mesh(source: &Mesh, min: Vec3, max: Vec3, gap: f32) -> Option<Mesh> {
    let mut mesh = source.clone();
    let positions = source.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
    let normals = source.attribute(Mesh::ATTRIBUTE_NORMAL).and_then(|v| match v {
        VertexAttributeValues::Float32x3(v) => Some(v), _ => None,
    });
    let span = (max - min).max(Vec3::splat(1e-6));
    let uv: Vec<[f32; 2]> = positions.iter().map(|p| [(p[0] - min.x) / span.x, (p[2] - min.z) / span.z]).collect();
    let positions: Vec<[f32; 3]> = positions.iter().enumerate().map(|(i, p)| {
        (Vec3::from(*p) + normals.map_or(Vec3::Y, |n| Vec3::from(n[i])) * gap).to_array()
    }).collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    Some(mesh)
}

// CODEX: verify fitting, Unicode rasterization, and pathological long words without a GPU.
#[cfg(test)]
mod tests {
    use super::*;

    // CODEX: maximum-length notes, spaces and unbroken words must fit the texture's writing region.
    #[test]
    fn long_messages_fit_and_have_ink() {
        let font = FontRef::try_from_slice(FONT_BYTES).unwrap();
        for text in ["Corn", "hello café", "first line\nsecond line", &"W".repeat(240), &"a little note ".repeat(20)] {
            let fitted = layout(text, 512, 0.12, 1024.0, &font);
            assert!(fitted.bottom - fitted.top <= 512.0 * 0.76);
            assert!(fitted.width <= 512.0 * 0.76);
            let bounds = pixel_bounds(&rasterize(text, false, false, 512, 0.12, 1024.0));
            assert!(bounds.width() > 0.0);
            assert!((bounds.center() - Vec2::splat(256.0)).abs().max_element() <= 2.0, "{text}: {bounds:?}");
            assert!(bounds.min.min_element() >= 59.0 && bounds.max.max_element() <= 453.0, "ink stays inside the padding");
        }
        assert!(layout("Corn", 512, 0.12, 1024.0, &font).px > 100.0, "short notes grow beyond the old 48px limit");
        assert!(pixel_bounds(&rasterize("Corn", false, false, 512, 0.12, 1024.0)).width() > 350.0);
    }

    // CODEX: typing must never turn "postit" into "post\n it", even at an oversized trial font.
    #[test]
    fn words_remain_whole_at_trial_and_fitted_sizes() {
        let font = FontRef::try_from_slice(FONT_BYTES).unwrap();
        for word in ["postit".to_string(), "W".repeat(240), "café".repeat(30)] {
            assert_eq!(lines(&word, &font.as_scaled(1024.0), 100.0), vec![word.clone()]);
            for text in [word.clone(), format!("hello {word} world")] {
                let fitted = layout(&text, 512, 0.12, 1024.0, &font);
                let rendered_words: Vec<_> = fitted.rows.iter().flat_map(|(line, _)| line.split_whitespace()).collect();
                assert_eq!(rendered_words, text.split_whitespace().collect::<Vec<_>>());
                assert!(fitted.width <= 512.0 * 0.76);
                assert!(fitted.bottom - fitted.top <= 512.0 * 0.76);
            }
        }
    }

    // CODEX: inspect rasterized pixels, so centering checks include the actual italic glyph extents.
    fn pixel_bounds(image: &Image) -> Rect {
        let size = image.texture_descriptor.size.width as usize;
        let mut bounds = Rect::EMPTY;
        for (i, pixel) in image.data.as_ref().unwrap().chunks_exact(4).enumerate() {
            if pixel[3] > 8 {
                bounds = bounds.union_point(Vec2::new((i % size) as f32, (i / size) as f32));
            }
        }
        bounds
    }

    // CODEX: balanced wrapping avoids a short orphan line and preserves intentional blank lines.
    #[test]
    fn wraps_at_spaces_and_retains_manual_line_breaks() {
        let font = FontRef::try_from_slice(FONT_BYTES).unwrap();
        let scaled = font.as_scaled(32.0);
        let width = measure("one two three", &scaled) + 1.0;
        assert_eq!(lines("one two three four", &scaled, width), ["one two", "three four"]);
        assert_eq!(lines("first\n\nsecond", &scaled, 500.0), ["first", "", "second"]);
    }

    // CODEX: blinking or finishing must leave every text pixel in place, including a single letter.
    #[test]
    fn cursor_is_an_overlay_and_starts_at_the_largest_size() {
        for text in ["A", "Corn", "first\nsecond"] {
            let editing = rasterize(text, false, true, 512, 0.12, 1024.0);
            let finished = rasterize(text, false, false, 512, 0.12, 1024.0);
            assert_eq!(editing.data, finished.data, "cursor-free layout must be unchanged on Enter: {text}");
        }
        assert!(pixel_bounds(&rasterize("", true, true, 512, 0.12, 1024.0)).height() > 380.0);
    }

    // CODEX: a manual size decrement begins at the actual fit and stays close to the requested step.
    #[test]
    fn manual_size_starts_at_the_current_fit() {
        let text = "A little sunshine\nfor your day";
        let current = fitted_font_size(text, 512, 0.12, 1024.0, FONT_BYTES);
        let smaller = fitted_font_size(text, 512, 0.12, current - 4.0, FONT_BYTES);
        assert!((current - smaller - 4.0).abs() < 0.1, "size must step down from {current}, got {smaller}");
    }

    // CODEX: Ctrl arrows cross a wrap boundary; reaching the largest count then grows to maximum size.
    #[test]
    fn line_count_size_jumps_cross_wrap_boundaries_and_respect_manual_breaks() {
        let text = "one two three four five six seven eight nine ten";
        let size = fitted_font_size(text, 512, 0.12, 1024.0, FONT_BYTES);
        let rows = fitted_line_count(text, 512, 0.12, size, FONT_BYTES);
        let smaller = font_size_for_line_change(text, 512, 0.12, size, 1024.0, FONT_BYTES, false);
        let fewer = fitted_line_count(text, 512, 0.12, smaller, FONT_BYTES);
        assert!(smaller < size && fewer < rows, "Ctrl+Down: {size}/{rows} -> {smaller}/{fewer}");
        let larger = font_size_for_line_change(text, 512, 0.12, smaller, 1024.0, FONT_BYTES, true);
        assert!(larger > smaller);
        assert!(fitted_line_count(text, 512, 0.12, larger, FONT_BYTES) > fewer);
        let full = font_size_for_line_change(text, 512, 0.12, size, 1024.0, FONT_BYTES, true);
        assert!((full - size).abs() < 0.1);
        let manual = "alpha\nbeta";
        let full = font_size_for_line_change(manual, 512, 0.12, 30.0, 1024.0, FONT_BYTES, true);
        assert!(full > 30.0 && fitted_line_count(manual, 512, 0.12, full, FONT_BYTES) == 2);
        let unchanged = font_size_for_line_change(manual, 512, 0.12, 30.0, 1024.0, FONT_BYTES, false);
        assert!((unchanged - 30.0).abs() < 0.1, "Ctrl+Down cannot remove manual breaks");
    }

    // CODEX: all nine positions align visible ink inside the padding without changing font size.
    #[test]
    fn nine_positions_align_to_the_padded_note() {
        for row in 0..3 {
            for column in 0..3 {
                let image = rasterize_aligned("A\nBee", false, true, 512, 0.12, 64.0, FONT_BYTES, UVec2::new(column, row));
                let bounds = pixel_bounds(&image);
                let actual_x = [bounds.min.x, bounds.center().x, bounds.max.x][column as usize];
                let actual_y = [bounds.min.y, bounds.center().y, bounds.max.y][row as usize];
                let targets = [512.0 * 0.12, 256.0, 512.0 * 0.88];
                assert!((actual_x - targets[column as usize]).abs() <= 2.0, "column {column}: {bounds:?}");
                assert!((actual_y - targets[row as usize]).abs() <= 2.0, "row {row}: {bounds:?}");
            }
        }
    }

    // CODEX: gameplay letters remain text; backspace, Unicode, length limits and finish share one path.
    #[test]
    fn edits_text_and_reports_actions_without_gameplay() {
        use bevy::input::{keyboard::{Key, KeyboardInput}, ButtonState};
        let mut note = PaperText { editing: true, ..default() };
        let mut key = KeyboardInput { key_code: KeyCode::KeyE, logical_key: Key::Character("E".into()),
            state: ButtonState::Pressed, text: Some("WASD E R café!".into()), repeat: false, window: Entity::PLACEHOLDER };
        edit_key(&mut note, &key, 14, false);
        assert_eq!(note.text, "WASD E R café!");
        key.key_code = KeyCode::Backspace;
        edit_key(&mut note, &key, 14, false);
        assert_eq!(note.text, "WASD E R café");
        key.key_code = KeyCode::KeyA;
        key.text = Some("12345678".into());
        edit_key(&mut note, &key, 14, false);
        assert_eq!(note.text.chars().count(), 14);
        key.key_code = KeyCode::Enter;
        edit_key(&mut note, &key, 14, true);
        assert!(note.action.is_empty(), "Ctrl+Enter must not finish the note");
        assert!(!note.text.ends_with('\n'), "line breaks respect the character limit");
        note.text = "first".into();
        edit_key(&mut note, &key, 14, true);
        assert_eq!(note.text, "first\n");
        edit_key(&mut note, &key, 14, false);
        assert_eq!(note.action, "finish");
        key.key_code = KeyCode::KeyA;
        edit_key(&mut note, &key, 100, false);
        assert_eq!(note.text, "first\n", "no input after a completed action");
        note.action.clear();
        key.key_code = KeyCode::Delete;
        edit_key(&mut note, &key, 14, false);
        assert_eq!(note.action, "discard");
    }
}
