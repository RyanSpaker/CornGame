CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 07:56:49 PM EST 2026 — agent-maintained changelog (human DEVLOG.md is untouched)

Format: one `#` entry per chat, headed `CLAUDE|CODEX: <model> <reasoning> <date+time>`.
Each relevant turn is a `##` subheading with same format followed by a very short description.
Each entry keeps a summary (scope + why + what) of at most 20 lines, immediately after the top level heading.


# CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 07:56:49 PM EST 2026
Scope: demo/lobby (web build: demo/lobby/web/audio.js, launcher.js; src/voice_box.rs; assets/lobby.corn)
Why: web builds keep playing audio when the player isn't looking at or playing the game. Because browser throttles, this often leads to crackling audio. Additionally, loading textures hits wasm's single thread, also causing crackle.

What: on web, audio must be muted when any of these hold:
- the browser tab is not active (`document.visibilityState != "visible"`),
- the mouse/pointer is not captured (pointer lock released),
- the menu is visible.
Audio un-mutes only when none of them hold.
The mic permission prompt pauses the page (and so the voice box sound), so it is requested only once the sound has finished.

## Thu Nov  5 08:02:35 PM EST 2026
CLAUDE: claude-sonnet-5-5 low
Implemented web audio mute: audio.js `setAudioMuted`, launcher.js `syncAudio` (tab hidden / no pointer lock / menu visible).

## Thu Nov  5 08:47:34 PM EST 2026
CLAUDE: claude-sonnet-5-5 low
Voice box: the mic request waits for its sound to finish (`MicAfterSound` removed -> `request_microphone`).

## Thu Nov  5 08:55:14 PM EST 2026
CLAUDE: claude-sonnet-5-5 low
TODO in lobby.corn (voice_box): replace the despawn observer with one generator that waits for the sound or a timeout.

## Thu Nov  5 08:56:30 PM EST 2026
CLAUDE: claude-sonnet-5-5 low
Fix: audio played under the menu while loading; Bevy's own context resume is blocked while muted (audio.js).

# CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 2026
Scope: demo/lobby (new src/pipeline_watch.rs, registered in main.rs)
Why: the web build freezes briefly when a post-it is picked up and again when it is placed: render pipelines first requested mid-game are compiled synchronously on wasm's single thread.

What: detection only (no fix). Render-world warnings, active once no pipeline was created for 120 frames:
- LATE: a pipeline requested after settling (not precompiled), or still unready 3 frames later (native: draws skipped).
- STUTTER: pipeline building took > 4 ms in a frame (times `PipelineCache::process_queue`; meaningful on wasm).
Native check: pickup warns for 2 prepass variants + the shaders/post_it.wesl forward pipeline; placement warns for the alpha-blend ink pipeline + prepass variants.

## Thu Nov  5 2026
CLAUDE: claude-sonnet-5-5 low
Added pipeline_watch plugin; verified detector 1 live on native via corn-dev. Detector 2 (wasm) not yet exercised.

# CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 08:41:04 PM EST 2026
Scope: demo/lobby (new src/moon_shader.rs + assets/shaders/moon.wesl; src/scene.rs, src/main.rs, assets/lobby.corn)
Why: WebGL2 allows one directional light per view and one shadow-map layer shared by directional and spot lights. The lobby had two directional lights (moonlight, Sun) plus shadowed spots, so the overhead spot lost its shadow on web. The Sun only exists to light the moon's face; render layers cannot confine a light to one mesh (they filter by camera).

What:
- The Sun is no longer a light: `MoonSun` captures its colour/illuminance and removes its DirectionalLight; `MoonMaterial` (StandardMaterial + `MoonSunlight`) adds the Sun's Lambert diffuse after normal PBR, using the Sun's live direction (re-synced every frame, new materials start from the latest value).
- The moonlight is the only directional light; on wasm its shadow is off. On wasm only the overhead spot (`WebShadow`) keeps a shadow (`web_shadows` in scene.rs).

## Thu Nov  5 08:41:04 PM EST 2026
CLAUDE: claude-sonnet-5-5 low
Added moon_shader.rs/moon.wesl, `WebShadow`, corn authoring. Native compiles and starts, Sun captured once at 683 lux; moon render, several Sun directions and the wasm build are NOT yet checked.

## Thu Nov  5 2026 (later)
CLAUDE: claude-sonnet-5-5 low
Reworked pipeline_watch: detector 1 now scans the render phases for draws whose pipeline is not built (no "requested after 120 quiet frames" heuristic); detector 2 warns when a frame takes >2x the median of the last 60 and pipelines were built in it or the frame before. Armed by the game (src/pipeline_watch_arm.rs: player + scene `Ready` + web pointer lock), then by the first frame that draws with everything already built.

## Thu Nov  5 2026 (precompile)
CLAUDE: claude-sonnet-5-5 low
Removed pipeline_watch's "first clean frame" rule (it hid first-view misses). Added demo/lobby/src/precompile.rs: simplified copy of crates/pylon/src/precompile.rs (no WarmUpView/settings/report_misses), registered for StandardMaterial, MonitorMaterial, MoonMaterial, PaperMaterial. pipeline_watch tags each warning with whether precompile requested the pipeline. TODO in precompile.rs: extract pylon's precompile, this copy and pipeline_watch into a shared crate.

## Thu Nov  5 2026 (arm after precompile)
CLAUDE: claude-sonnet-5-5 low
pipeline_watch now also waits for `precompile::caught_up` (nothing waiting, a view setup known, every made pipeline Ok), latched, before it listens: the first view's pipelines are built the frame precompile first learns the camera's setup, which is loading, not a miss.

## Thu Nov  5 2026 (post-it warm-up, startup timeout)
CLAUDE: claude-sonnet-5-5 low
- demo/lobby/src/post_it_warmup.rs: requests the held-paper (`PaperMaterial`) and typed-ink (blend `StandardMaterial` + ink mesh) pipelines during loading via `PrecompileQueue`, built through new shared `paper_material`/`ink_material` (post_it_material.rs; copy_paper/render_text use them). TODO inside: do this when looking at the post-its instead of at load.
- pipeline_watch: 120-render-frame startup timeout after arming; past it, warns what precompile still waits on (`precompile::progress`) and listens anyway.
- pipeline_watch_arm.rs: TODO for extending the one-shot startup gate to scenes spawned later or alongside other loaded scenes.
Native check: precompile caught up after 50 frames; 4 simulated pickups + typed text produced no pipeline_watch warning.

# CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 10:16:13 PM EST 2026
Scope: crates/corn_scripting (src/math.rs, src/live.rs, src/lib.rs)
Why: a script wrote NaN into `Player.yaw` and the game panicked a frame later in bevy (`Dir3::new_unchecked`: vector not normalized, length NaN), far from the offending call. Scripts shouldn't be able to crash the game this way.

What: values a script writes into the world are checked at the host's write path, and refused with a catchable JS error (the value is left unchanged):
- non-finite (NaN/infinite) floats: any float field via `live::write`, and any Vec3/Quat/Transform (in-place `Slot::update`, whole-value `live::write`, `host_insert_math`).
- rotations: zero length refused; normalized once they're in the world. A script's own (not yet inserted) Quat isn't normalized per write, so `q.x`, `q.y`... can still be built one at a time.
Caveat: writing a live rotation's components one at a time (`component(e,"Transform").rotation.x = ..`) normalizes after each write; assign a whole Quat instead.

## Thu Nov  5 10:16:13 PM EST 2026
CLAUDE: claude-sonnet-5-5 low
Added `Math::sanitized`, a `Checked` trait behind `Slot::update`, float check in `live::write`; 5 unit tests (21 pass). Verified live in the lobby: NaN yaw, NaN/inf vectors, zero quat and a NaN Transform insert all throw and the game keeps running.

## Thu Nov  5 2026 (startup report)
CLAUDE: claude-sonnet-5-5 low
pipeline_watch logs one INFO line when precompile catches up (or the 120-frame timeout fires): pipelines precompiled, render frames and seconds since the first render frame, frames since arming, how many are drawn that frame, and how many drawn pipelines were not precompiled. Native: 106 precompiled in 34 frames / 2.13 s, 66 drawn, 0 unprecompiled. Uses new `precompile::made`.

## Thu Nov  5 2026 (usage report)
CLAUDE: claude-sonnet-5-5 low
pipeline_watch keeps every pipeline any draw has needed (`ever_drawn`) and, every 30 s after startup, logs "N of M precompiled pipelines have been drawn so far (K never)" plus how many drawn pipelines were not precompiled, only when N changed; the never-drawn ones are listed at debug level. Native: 70/106 after 30 s, 76/106 after a pickup + typed ink, 0 not precompiled, 0 warnings.

## Thu Nov  5 2026 (startup report timing)
CLAUDE: claude-sonnet-5-5 low
Removed the 30 s usage log. The single startup report now runs one frame after precompile finishes (or the timeout fires) and counts pipelines used so far (every pipeline any draw has needed since the first frame), not just that frame's. Native: 106 precompiled in 41 frames / 2.23 s, 67 used so far, 0 not precompiled.

# CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 2026 (camera first frame)
Scope: demo/lobby (src/controller.rs)
Why: the scene spawns between Update and PostUpdate, so the player camera rendered its first frame at identity rotation (cabin wall head-on) until `walk` ran a frame later (web: until pointer lock); startup pipeline compiles kept that frame on screen ~0.8 s.
What: `face_spawn_view` (PostUpdate, before transform propagation) sets the rotation from `Player` yaw/pitch on `Added<Player>`; marked TODO hack.

## Thu Nov  5 2026
CLAUDE: claude-sonnet-5-5 low
Added `face_spawn_view`.

## Thu Nov  5 2026 (camera switch)
CLAUDE: claude-sonnet-5-5 low
`face_spawn_view` can be switched off from cornscript: `resource("FaceSpawnView").on = false` (default on).

## Thu Nov  5 2026 (camera switch as Player field)
CLAUDE: claude-sonnet-5-5 low
Replaced the `FaceSpawnView` resource with `Player.face_spawn_view` (default true); lobby.corn sets it false, so the first frame is currently as the scene spawns it.


# CODEX: GPT-6 high — Thu Nov 5 2026
Scope: demo/lobby (assets/stuck_player_key_helper.corn, assets/lobby.corn, src/interaction.rs)
Why: help players who see the small key but struggle to aim precisely enough to pick it up.
What: standalone corn helper owns hint copy, thresholds, missed-E counting and timed visibility episodes.
The second missed E shows a hint; later presses repeat only after dialogue finishes. Full disappearance or pickup resets the count.
The timed hint requires >12 seconds, >=2 seconds of cursor visibility, cursor presence within the first 6 seconds, and a currently visible cursor.
Timed lookaway grace is 3 seconds before firing and 0.5 seconds afterward; longer gaps or pickup reset the episode.
Generic reflected InteractionFeedback exposes per-item cursor visibility, activation and unused E presses; Message exposes actual completion.

## Thu Nov 5 2026 — small-key helper
Added the include and configurable coroutine; checked helper behavior with simulated inputs and added interaction/scene syntax regressions.

## Thu Nov 5 2026 — timed helper tuning TODO
Added a TODO beside the second helper settings to playtest and tune timing, cursor thresholds and text.

## Thu Nov 5 2026 — helper message progression and hold
Each helper advances independently through its text array when shown, retaining the last line and preserving progress across resets. Helper-only post-typing hold is now 2 seconds (was 2.5), tunable via holdSeconds in stuck_player_key_helper.corn; Say accepts an optional hold override.

## Thu Nov 5 2026 — faster reset after timed hint
Added tunable lookawayAfterHint (0.5 seconds): after firing, a longer lookaway starts a fresh timed episode; before firing the 3-second grace remains. Checked both strict boundaries and message progression.

# CODEX: GPT-6 high — 2026-11-06
## Table and radio barrel use trimesh colliders
Scope: demo/lobby/assets/lobby.corn.
Why: dropped props sustain contact jitter on the generated convex hulls; live table-trimesh tests settled.
What: use TrimeshFromMesh for tabletop and barrel_freestanding, including its nested Cylinder override.
TODO: fix convex-hull contact instability or create authored colliders; retry convex hulls after Avian switches to Peck.
Validation: restarted the native game; original flashlight settled and slept on both surfaces with zero final height jitter.
Confirmed the barrel's three mesh colliders attach to its nested Cylinder; restored prop poses and mouse control after checks.

# CODEX: GPT-6 high — 2026-11-05
## Black lobby window before the first rendered frame
Scope: demo/lobby startup_window module, main registration, Linux dependencies, Cargo.lock.
Why: i3 can expose undefined X11 contents outside the original window size before rendering.
What: create hidden, install a black X11 background on the main thread, then show normally;
use a black global GPU fallback while the scene camera loads. Isolated from web builds.
Documentation: .agents/doc/startup-window.md explains setup, limits, tuning, and why delaying creation is insufficient.
Validation: native check/build and module rustfmt passed; Xvfb/i3 first-map probe found all 1600x1000 pixels black.
Later GPU presentation under Xvfb was unavailable (no DRI3/supported present modes).
