CLAUDE: claude-sonnet-5-5 low — Thu Nov  5 07:56:49 PM EST 2026 — agent-maintained changelog (human DEVLOG.md is untouched)

Format: one `#` entry per chat, headed `CLAUDE|CODEX: <model> <reasoning> <date+time>`.
Each relevant turn is a `##` subheading with same format followed by a very short description.
Each entry keeps a summary (scope + why + what) of at most 20 lines, immediately after the top level heading. This must be kept accurate. Consider updating **after every turn.**

NEW ENTRIES GO TO THE **END** OF THE FILE.

# CODEX: GPT-6 high — Fri Nov 06 2026 07:00:06 AM EST
Scope: lobby camera zoom and one-time post-it teaching tooltip.
Why: make post-it clues easier to read and teach the right-click control.
What: hold right click for true 1.5x magnification, interpolated linearly over 0.7s; release returns over the same duration. Computer focus, paper editing/carrying, radio tuning and pause block zoom. Visible stack, static Blender post-its and player-posted notes show the shared teletype-styled "right click to zoom" hint, hidden for the rest of the session after full zoom plus 0.25s continuously held. ZoomSettings exposes magnification, timing, hint size and placement to the dev console. Added isolated zoom module and four headless regression tests.

## CODEX: GPT-6 high — Fri Nov 06 2026 07:00:06 AM EST — implementation and verification
All 61 lobby tests and the native build passed. Regression checks cover linear projection magnification, transition/hold timing, partial-release reset, input ownership, authored/runtime note targeting, occlusion and permanent session dismissal. Formatting and whitespace checks passed. Visual launch could not run because XOpenDisplay failed in this environment.

## CODEX: GPT-6 high — Fri Nov 06 2026 07:02:09 AM EST — right-click correction
Changed the zoom button and teaching hint to right click per user correction; timing and completion behavior unchanged. All four focused zoom tests and formatting checks passed.

# CODEX: GPT-6 high — Fri Nov 06 2026 06:23:12 AM EST
Scope: lobby radio message audio and its isolated generator.
Why: encode the requested clue as spoken decimal ASCII.
What: set editable MESSAGE_TEXT to "sorry. no msg yet. good work." and derive all 29 ASCII codes, preserving spaces and punctuation. Regenerated only message.ogg with the existing filtered voice and two-second end pause; normalized to the existing -23 LUFS target.

## CODEX: GPT-6 high — Fri Nov 06 2026 06:23:12 AM EST — ASCII message regeneration
Verified ASCII round-trip, successful full decode, mono 22050 Hz Vorbis, 68.590s duration and 2.000s ending silence. Measured -22.94 LUFS and -2.15 dB true peak. No listening verification.

## CODEX: GPT-6 high — Fri Nov 06 2026 06:26:11 AM EST — relocated generator and revised clue
Moved the generator beside the radio sounds and adjusted its output directory. Swapped near/drone synthesis to match the user's asset swap, renamed drone controls, and updated the CODEX radio documentation. Removed generation for authored stations 1/2/3/5, retaining the original station_4 melody; added selective drone/station_4 generation. Verified all full/selective output routes with encoding mocked. Updated the clue to the final supplied "sry. no msg yet. good work" (26 decimal ASCII codes, no final period) and regenerated only message.ogg: mono 22050 Hz Vorbis, 61.565s, 2.000s ending silence, -22.94 LUFS, -2.06 dB true peak. Verified hashes of every other OGG were unchanged. No listening verification.

## CODEX: GPT-6 high — Fri Nov 06 2026 — correction: ASCII belongs in numbers.ogg
Previous turns incorrectly overwrote the user's authored message.ogg (original observed size 1,300,277 bytes). Copied the verified latest ASCII recording into numbers.ogg and corrected the generator: NUMBERS_TEXT/CODES/PAUSE_SECONDS, --only numbers, and no message generation in any route. Updated RADIO.CODEX.md and verified full/selective routes with mocked encoding. The normalization backup contains an older synthesized 17.121s message, not the authored recording, so it was not used as a false restoration. Authored message restoration remains unresolved pending a source copy; asked user for its location. Other sound assets were not modified in this correction.

## CODEX: GPT-6 high — Fri Nov 06 2026 — authored message restored from ZFS snapshot
Restored message.ogg from the user-specified /.zfs/snapshot/zfs-auto-snap_frequent-2026-11-06-06h15 copy. Verified exact byte equality: 1,300,277 bytes, SHA-256 0ac1693b28f726d1f4f7381e5329699a8cf9b58cd9799ce67490937e7a73626f. Verified hashes of all other OGG files unchanged, including the corrected ASCII numbers.ogg. Authored message restoration is complete.

# CODEX: GPT-6 high — Fri Nov 06 2026 05:36:49 AM EST
Scope: art/lobby.blend radio clues on the two freestanding barrels.
Why: direct the player from radio_spawn toward barrel_near_region and illustrate the relevant tuning positions.
What: added an exportable black lid arrow pointing toward barrel_near_region and a yellow post-it with a circle dial, eleven labeled ticks from 0.0 to 1.0, blue highlights at 0.1/0.2/0.3/0.5/0.8 and pink at 0.2/0.9. Pointer is at 0.9; geometry matches radio.corn's clockwise ±150-degree sweep and +X rest marker. Isolated additions in CODEX_radio_clues, parented them to their barrels, and embedded the annotated generator as CODEX_radio_clues.py. Note root exposes size/sweep/tuning provenance and supports whole-note transform adjustments.

## CODEX: GPT-6 high — Fri Nov 06 2026 05:36:49 AM EST — Blender implementation and visual verification
Inspected both additions in material preview; verified readable labels, dual-color 0.2, pointer alignment, and arrow placement. Counted eleven mesh labels and eleven ticks; all graphics are meshes for future export. Saved the open art/lobby.blend. No game export requested or performed.

## CODEX: GPT-6 high — Fri Nov 06 2026 — authored desk notes with image overlays
User refined scope: create two new post-its on the lobby desk, touching no other notes. Added CODEX_desk_dial_postit and CODEX_desk_map_postit as exact geometry copies of postit1/postit2 at their original size, retaining the source paper material/atlas through independent material copies. Applied separate transparent 256×256 PNG overlays with dedicated UVs; packed both into lobby.blend and saved PNGs in art/CODEX_radio_{dial,regions}_256.png. Dial keeps the original calibration/highlights. Map uses current region affine transforms (including offset near falloff), four region labels/frequencies, solid clear/dashed falloff outlines, and the cabin wall footprint. Existing notes and barrel arrow were untouched. Visually inspected both curved notes in isolation, verified geometry matches the source meshes and both packed image dimensions, restored full lobby view, and saved lobby.blend. Isolated generators are embedded as CODEX text blocks and retained under .agents/tmp.

## CODEX: GPT-6 high — Fri Nov 06 2026 — corrected desk-note artwork
Rotated dial artwork so midpoint 0.5 is straight up, preserving the clockwise 300-degree sweep and 0.9 pointer; removed all numeric dial labels per the follow-up, retaining eleven ticks and existing highlights. Changed the message region's upper semicircle to blue on both clear and falloff boundaries; lower semicircle remains pink. Regenerated the 256×256 PNGs, visually checked the artwork, reloaded/repacked both Blender images, updated embedded source scripts, and saved lobby.blend. Meshes, placements, and other notes were untouched.

## CODEX: GPT-6 high — Fri Nov 06 2026 — thicker strokes and simplified map
Doubled all dial/map stroke widths, enlarged map center dots, and lengthened falloff dashes for readability at 256×256. Removed all four region-name/frequency labels; retained cabin, direction, title, and boundary legend. Preserved the upward dial midpoint, 0.9 pointer, highlights, and half-blue message outline. Visually inspected regenerated PNGs, refreshed packed Blender images and embedded generators, and saved lobby.blend without geometry or placement changes.

## CODEX: GPT-6 high — Fri Nov 06 2026 — ground-aligned map without annotations
Removed all remaining map text, the cabin label leader, and the +Y direction arrow. Mapped actual ground mesh world XY bounds (-26.3251,-22.6622)–(24.9565,28.6194) directly to the full 256×256 overlay, so post-it edges correspond to ground edges; clipped regions outside those boundaries. Verified opposite corners map to (0,256)/(256,0), visually checked the PNG, refreshed the packed image/embedded generator, and saved lobby.blend. Preserved user-adjusted note transforms and all other scene objects.

## CODEX: GPT-6 high — Fri Nov 06 2026 — narrower dial highlights and heavier map
Reduced colored dial bands from 14px to 7px while retaining thicker black pen strokes. Increased map outline widths another 50% (stroke scale 2→3), preserving the text-free, arrow-free ground alignment and half-blue message region. Visually checked both 256×256 PNGs, refreshed/repacked Blender images and embedded generators, and saved lobby.blend. No object or placement changes.

## CODEX: GPT-6 high — Fri Nov 06 2026 — third desk post-it with station list
Added CODEX_desk_stations_postit beside the two current clues using an exact geometry copy of authored postit3, original paper atlas/material copy, and a separate packed 256×256 transparent image. Six lines reproduce the latest user text without a heading: 90.1 music; 90.2 news?; 90.3 broadcast; 90.5* easy listening; 90.8 CHKN; 90.9 ???. Preserved other note placements. Visually inspected the PNG and new note on the desk, verified source geometry and packed image dimensions, embedded isolated generators, and saved lobby.blend.

## CODEX: GPT-6 high — Fri Nov 06 2026 — padded ground border on map
Inset ground bounds by 5% per side (12.8px on the 256×256 overlay), rescaled cabin/region artwork consistently, and added a 3px black ground-boundary rectangle. Regions clip to the border, leaving the paper margin clear. Visually verified PNG and corner mapping at 12.8/243.2px, repacked the map image, refreshed embedded source/metadata, and saved lobby.blend. Other artwork and scene transforms remain unchanged.

## CODEX: GPT-6 high — Fri Nov 06 2026 — filled regions, black cabin, starting-radio dot
Filled all inner radio regions, preserving the half-blue/half-pink message disk; removed redundant center dots. Filled the cabin footprint pure black and added a small 2.5px-radius red dot at the current radio_spawn world XY position using the same ground-to-image mapping. Retained 5% padding, ground border, and dashed outer falloff boundaries. Visually inspected the 256×256 PNG, repacked it, refreshed the embedded generator, and saved lobby.blend without changing note transforms or other artwork.

## CODEX: GPT-6 high — Fri Nov 06 2026 — bold padded station list
Increased station-list left padding from 20px to 32px and changed Adwaita Mono to bold (weight 700), preserving the six supplied lines and 256×256 size. Visually checked the PNG for clipping, refreshed the packed Blender image and embedded generator, and saved lobby.blend without scene placement changes.

## CODEX: GPT-6 high — Fri Nov 06 2026 — red radio X
Replaced the map's starting-radio dot with a 6px red X using 2px rounded strokes at the same radio_spawn-derived coordinates. Visually checked the regenerated 256×256 PNG, refreshed the packed image/embedded generator, and saved lobby.blend.

## CODEX: GPT-6 high — Fri Nov 06 2026 — thicker dial highlights
Increased dial color-band width from 7px to 10px, retaining black stroke widths, calibration, pointer, and blue/pink assignments. Visually checked the regenerated 256×256 PNG, refreshed the packed dial image and embedded generator, and saved lobby.blend.

## CODEX: GPT-6 high — Fri Nov 06 2026 — authored paper and separate ink meshes for glTF
Investigated black runtime clues by comparing Blender graphs, authored tabletop_postit_custom_* objects, and both source/runtime glTF materials. The unsupported paper/transparent-ink mix exported only the ink image as an opaque paper base color. Following the user's correction, restored paper-only material copies from the existing postit1/2/3 assets and added separate child ink objects. Dial/map use curled overlay meshes with directly connected 256px RGBA textures and glTF BLEND materials; the station list uses a separate bold glyph mesh sharing the authored postit_handwritten_ink material. Kept user-adjusted note transforms and all other authored notes intact.
Added demo/lobby/tools/make_postit_ink_meshes.py and an embedded copy with tunable ink offset/text metrics. Re-exported art/lobby.gltf with the saved authoring settings and refreshed only lobby runtime assets through prepare_assets.export. Verified the actual exported three opaque paper materials reference the authored paper atlas, each paper node owns its separate ink child, station glyphs share the authored ink material, and both graphic children export BLEND alpha. Runtime subset has 651 nodes; image references resolve. Removed the discarded composite bake utility/textures. Automatic review rejected a temporary Bevy camera/exposure check after the user rejected that approach; it was not performed. Stopped the verification instance. Saved lobby.blend; no claim of a final Bevy visual check.

## CODEX: GPT-6 high — Fri Nov 06 2026 — station lettering follows authored paper modifier
Fixed only CODEX_desk_stations_postit_text: copied every writable setting of the authored lettering's Shrinkwrap modifier and retargeted it to the station-list paper. Uses PROJECT along negative local Z, ON_SURFACE, offset 0.00012m, matching tabletop_postit_custom_03_text exactly. Updated the isolated generator and embedded copy so new lettering gets this modifier. Checked all 7,565 evaluated vertices against the paper: maximum distance 0.000120007m. Verified exported glTF glyph surface heights are on the paper rather than 0.04m above it, refreshed runtime lobby assets, and saved lobby.blend. Dial/map unchanged.

## CODEX: GPT-6 high — Fri Nov 06 2026 — corrected two-color map region
Moved the blue/pink split from message to numbers for both filled inner disks and dashed outer boundaries; message is entirely pink. Visually checked the 256×256 PNG, refreshed the packed map image/embedded generator, updated the exact texture paths referenced by source/runtime glTF, and saved lobby.blend. Separate ink meshes, station shrinkwrap, and transforms remain unchanged.

# CODEX: GPT-6 high — Fri Nov 06 2026 04:50:00 AM EST
Scope: demo/lobby self-contained regional radio attenuation dev UI.
Why: inspect region geometry and linear strength in-game with a temporary Ctrl-/ shortcut.
What: added radio_dev_ui.rs using existing Bevy UI and isolated procedural map drawing, without a new framework. Bottom-left panel defaults to 20% window width, fits the authored ground mesh, draws inner/outer boundaries and centers, green spatially active/yellow inactive regions, shared affine falloff segments, player dot and radio square. Shows tuning, power and every active region's frequency, name, audio file and linear strength. RadioDevUi exposes live size, padding, font and marker controls through the dev console. Registers from the radio plugin; shares exact intersection math with radio_region.rs.

## CODEX: GPT-6 high — Fri Nov 06 2026 04:50:00 AM EST — regional overlay implementation
All 21 radio tests pass, including a new offset/sheared-boundary test verifying the dev segment endpoints against the audio strength. Native build and WebAssembly check pass. Focused real-scene smoke passes: Ctrl-/ open/close, active region name/audio path/strength and screenshot capture. Visually inspected /tmp/radio-ui/radio-dev-ui.png.

# CODEX: GPT-6 high — Fri Nov 06 2026 04:38:16 AM EST
Scope: demo/lobby radio audio assets and a reusable normalization utility.
Why: user approved normalizing all ten unique radio recordings to -23 LUFS / -1 dBTP after the loudness survey.
What: backed up originals and aliases under demo/lobby/.agents/tmp/radio-normalization-20261106-043628; applied constant gain and Vorbis quality-5 encoding. Preserved dynamics, exact durations, rates, channel counts and station_3/station_5 symlinks. Added tools/normalize_radio_audio.py (read-only by default; --apply requires prior user approval), staged verification and a before/after report with original/normalized hashes. Runtime volume knobs remain unchanged.

## CODEX: GPT-6 high — Fri Nov 06 2026 04:38:16 AM EST — approved audio normalization
All ten unique outputs fully decode and measure -23.09 to -22.95 LUFS; highest true peak -1.20 dBTP. Confirmed installed stream geometry/codec, original backup hashes and both symlinks. Static needed a corrective constant-gain pass after lossy encoding shifted loudness. No compression/limiting was needed, and the incomplete first staging attempt was discarded after verifying the final backups. No in-game listening review.

# CODEX: GPT-6 high — Fri Nov 06 2026 04:28:23 AM EST
Scope: demo/lobby radio broadcast/static balance controls and read-only audio loudness survey.
Why: normal stations and regional signals need one shared volume knob independent of static; user requires prompting before audio-file changes.
What: added reflected Radio.broadcast_gain (default 1), applied to both broadcast kinds while preserving individual gains. Grouped broadcast_gain, existing static_gain and master volume in RADIO_AUDIO in radio.corn. Audio files remain unchanged; normalization requires explicit approval after measurements.

## CODEX: GPT-6 high — Fri Nov 06 2026 04:28:23 AM EST — separate broadcast and static controls
All 20 radio tests pass. Extended the runtime mixer test to cover normal/regional source trims, live broadcast changes with unchanged hiss, live static changes with unchanged broadcasts, and the existing power fade. Documented scene-load variables versus live reflected fields. Measured all ten unique radio files with FFmpeg; proposed constant-gain normalization to -23 LUFS / -1 dBTP, pending approval.

# CODEX: GPT-6 — Fri Nov 06 2026 04:15:06 EST
Scope: demo/lobby radio audio assets.
Why: Bevy rejected Opus audio stored in Ogg files.
What: Converted chicken_on_a_raft.ogg and war_of_the_worlds_compressed.ogg to Vorbis quality 5. Preserved sample rates, channels, filenames, and station_3/station_5 symlinks; retained existing Vorbis assets.

## CODEX: GPT-6 — Fri Nov 06 2026 04:15:06 EST — radio Opus conversion
Both converted recordings passed a complete FFmpeg decode. All 12 radio paths, including linked stations, report Vorbis through ffprobe.

# CODEX: GPT-6 high — Fri Nov 06 2026 03:55:47 AM EST
Scope: demo/lobby first-pass radio reception, reflected tuning controls and radio documentation.
Why: exact-center tuning and equal-power crossfading made stations fade instead of staying loud under interference.
What: linear dial proximity feeds independent acquisition [0.10, 0.45] and clarity [0.25, 0.75] smoothsteps; removed mix normalization. Regional spatial strength feeds both curves before gain, preserving 0.85 normal suppression, interference hiss and layered secrets. Kept existing smoothing, controls, preload and recordings. Exposed threshold pairs in Radio/radio.corn. Fixed output volume is 0.5 for measured additive headroom; processing/compression remains a later milestone.

## CODEX: GPT-6 high — Fri Nov 06 2026 03:55:47 AM EST — forgiving tuning and independent hiss
Verified 55 lobby unit tests (20 radio tests), including flat clean center, full broadcast gain under hiss, continuous/symmetric tuning, independent controls, overlapping regions, spatial edges, mixer gains and power fade. WebAssembly check passes. Decoded station 2 peaks at 1.327394; a coefficient-aware 0.001-step sweep bounds station/message/static overlap at 1.834007 before output volume, or 0.917004 at the new fixed 0.5 volume. No in-game listening verification; filtering, crackle/dropouts and combined-bus compression deferred.

# CODEX: GPT-6 high — Fri Nov 06 2026 02:44:36 AM EST
Scope: demo/lobby radio controls, shared held-rotation timing, radio recordings/generator, and deferred audio requests.
Why: correct dial direction and power behavior; face held radios toward the player; revise requested broadcasts; avoid downloading unused audio before tuning is introduced.
What: clockwise dial motion increases tuning to 100%; F preserves frequency and knob position.
T turns only held radios toward the player's full yaw/pitch direction, without roll; tipped pickup correction remains upright and yaw-only.
Both use the shared quintic-eased quaternion transition: 0.2s delay followed by a 0.5s turn; retargeting preserves the current angle and carry-bounds basis.
Regenerated near.ogg without random noise, using four pronounced 2.5s intensity swells per 10s loop; exposed synthesis controls and selective generation.
Regenerated message.ogg with ten spoken numbers and exactly 2s ending silence; subsequent audio revisions are recorded in their respective chat entries.
Joined ~/dump/nprcorn/1.mp3, 2.mp3 and 3.mp3 in order with 1s silence between clips, replacing station_2.ogg (11m 52s).
Initially request only the default station (station 4 at 50%); the first visible tuning tooltip permanently unlocks remaining station, static and regional audio requests.
Validation: radio regression tests passed after each code change (22 at the final pitch change); generated audio was decoded/measured. No live browser-network or final visual/listening verification.

## CODEX: GPT-6 high — Fri Nov 06 2026 02:44:36 AM EST — radio controls and tuning orientation
Verified all 50 lobby unit tests with `cargo test -p lobby --bin lobby --offline`, including clockwise model-space rotation, preserved frequency, T input, parented placed turns, and held retarget continuity. No live visual verification.

## CODEX: GPT-6 high — Fri Nov 06 2026 02:46:57 AM EST — shared rotation timing
Set radio correction/tuning turns to 0.5 seconds after a shared 0.2-second pause. Kept placed-turn cleanup aligned with delayed completion; updated carry and placed tests to check the pause. All 50 lobby unit tests pass (`cargo test -p lobby --bin lobby --offline`).

## CODEX: GPT-6 high — Fri Nov 06 2026 02:51:15 AM EST — face radio on tuning only while held
Guarded T-facing input and its observer with Held; removed the placed-radio turn system. Kept placed tuning/mouse control and held 0.2s delay/0.5s rotation. All 14 radio tests pass (`cargo test -p lobby --bin lobby radio --offline`), including the placed-tuning orientation regression.

## CODEX: GPT-6 high — Fri Nov 06 2026 02:56:54 AM EST — regenerate near-source audio
Original make_radio_audio.py mixed filtered seeded random noise with a gently modulated 91 Hz sine. Replaced near.ogg with a noise-free 91 Hz harmonic drone, four deep 2.5s swells per 10s loop; exposed synthesis constants and added --only near. Other audio files untouched. Verified periodic synthesis, mono 22050 Hz Vorbis decode, 10s duration, 0.214 decoded peak, and 22.4 dB swell range. No listening verification.

## CODEX: GPT-6 high — Fri Nov 06 2026 02:58:24 AM EST — ten-number message loop
Regenerated only message.ogg as ten spoken numbers (4, 8, 15, 16, 23, 42, 7, 19, 31, 56), with the original radio-filtered espeak voice. Trimmed its TTS tail before padding exactly 2s; added --only message and editable content/pause constants. Verified ten entries, mono 22050 Hz Vorbis decode, 17.121s total duration and 2.000s ending silence via ffmpeg. No listening verification.

## CODEX: GPT-6 high — Fri Nov 06 2026 03:26:47 AM EST — replace station 2 recording
Joined ~/dump/nprcorn/1.mp3, 2.mp3, 3.mp3 in numeric order with exactly 1s generated silence between clips; replaced demo/lobby/assets/sounds/radio/station_2.ogg with mono 44100 Hz Vorbis (quality 4), preserving the existing station path.

## CODEX: GPT-6 high — Fri Nov 06 2026 03:30:47 AM EST — defer optional radio audio until tuning hint
Initially request only the normal broadcast nearest the startup dial (station 4 at 0.5). The first visible secondary radio/tuning hint permanently unlocks requests for all remaining stations, static and regional sources for the session. Added integration coverage using actual AssetServer handles and hint visibility: no optional voices before the hint or while the computer UI blocks it; all load afterward without duplication. All 15 radio tests pass (`cargo test -p lobby --bin lobby radio --offline`); browser network behavior not live-verified.

## CODEX: GPT-6 high — Fri Nov 06 2026 05:25:21 AM EST — include pitch when tuning held radio
T now captures a full player-facing yaw/pitch target with level horizontal edges. Tipped pickup correction remains upright and yaw-only; shared delay/easing unchanged. Added observer-level tests for elevated/lowered and directly overhead/underneath player positions, checking both pickup and T targets. All 22 current radio tests pass (`cargo test -p lobby --bin lobby radio --offline`); no live visual verification.

# CODEX: GPT-6 — Fri Nov 06 2026 02:26:41 AM EST
Scope: demo/lobby/assets/lobby.corn camera startup.
Why: use the current running game's camera view at startup instead of the authored spawn.
What: OVERRIDE_SPAWN_VIEW (default true) applies captured position/yaw/pitch while building the scene; false preserves the existing spawn values. Enabled Player.face_spawn_view for correct rotation on the first rendered frame.

## CODEX: GPT-6 — Fri Nov 06 2026 02:26:41 AM EST — captured initial camera
Verified both override settings using the exact scene JS on temporary entities in the running game's console; cleaned up immediately. Existing running player was left unchanged; startup rendering was not restarted or visually verified.

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

# CODEX: GPT-6 high — Fri Nov 06 2026 17:20:13 EST
Scope: demo/lobby client networking, ordinary Door/KeyState, scratch Python server and agent-owned usage documentation.
Why: network the lobby at 10 Hz while keeping gameplay client-side, voice independent, and offline removal/disable simple.
What: added network.corn plus isolated std::net/browser WebSocket, physics, media and orb/label adapters; no game networking crate.
One Python stdlib process serves /lobby state and /voice P2P signaling, durable notes/radio/reconnect records, cached IP-town lookup and a 15s empty reset.
Shared state covers eye-height orbs/count, placed note text/style/selectors, radio holding/power/tuning, both keys/door, held flashlight beams and computer cursor/mouse/click/UI.
Awake dynamic bodies neither send nor apply poses. Buffered interpolation handles eligible poses; sleeping corrections use Avian tick bookkeeping without forcing sleep.
Door/key state is ordinary ECS. Initial open snapshots seek to the clip end; live opening animates. Key-slot parent/local pose is separate.
Paper visuals reuse CopyPaper with a default-false visual_only flag, preserving local carry/editor ownership. unique_selector is isolated in network.corn.
Reconnect tokens are stored locally; the server restores saved logical player poses. Welcome state is retained through asset loading and consumed explicitly because reflection list patches do not truncate.
Networking defaults off without LOBBY_SERVER / browser server=, and NetworkSettings.enabled disables it live. Added NETWORK.CODEX.md and focused protocol/audio/scene tests.

## CODEX: GPT-6 high — Fri Nov 06 2026 17:20:13 EST — implementation and verification
Final full lobby suite: 68 tests pass, including six focused networking tests. A transient concurrent word-filter test failure was resolved by its separate work; networking did not alter that code.
Six Python server tests pass, including real TCP voice/state routes, identity replacement/persistence, terminal duplicate-window replacement, ownership, reset boundaries and mocked town caching.
Real native/Chromium Opus test passed: bidirectional audio, stereo direction, distance attenuation and mute through the Python /voice route (generated tones; no physical microphone).
Two full native clients passed door initial/live playback, exact slot key, live note/style/attachment/deletion, local-only carried paper, radio handoff/power/tuning, held-only flashlight beam, computer UI/click/cursor, reconnect identity/pose and live offline disable.
Native build and WebAssembly check passed. Xvfb lacked Vulkan presentation; scene verification used the existing display with isolated server/ports/identities and cleaned up its processes.
Geolocation lookup logic was mocked in tests; no real visitor IP lookup or internet/NAT media traversal was exercised. Browser game rendering was compile-checked; browser voice was exercised acoustically.

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026
Scope: lobby post-it rejection, crumple sound.
Why: replace the hand-made bad-word list with BurntRouter/filtered-word-lists; crumple should be positional.
What: post_it.rs filters on Enter ("finish" becomes "reject") using src/word_lists/{blanket,standalone}.txt: blanket words match as substrings after removing whitespace (spaced-out slurs caught), standalone words only as whole words, words in both lists are standalone only (upstream blanket has "nig", which matched "night"). Aho-Corasick + hash set. lobby.corn drops BAD_WORDS, treats "reject" like a bad note; crumple sound is spatial at the note, rejection delay 1s. Test: 104 ordinary messages, 0 flagged. Known: removing spaces makes "Hancock street" match "cocks".

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026
Scope: lobby HUD text sizing (tooltips, bottom hints, radio/post-it/zoom hints).
Why: HUD was too large on web (logical window 864px tall at DPR 2.5) and would be on any screen below 4K.
What: added HudScale (logical window height / 1706 (the dev's 4K monitor is 2132px physical at OS scale 1.25), exactly 1.0 there so native is unchanged) and HudSize, applied in PostUpdate before UI prepare; tooltip placement and zoom hint use the scale too. Reasoning in .agents/doc/hud_scale.md. Lobby tests pass (68+10); wasm check compiles. Not visually verified.

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 06:15 PM EST
Scope: lobby server deployment (server.py socket activation, build_web.py, host nix module).
Why: run server.py on sonoran.xyz with no per-app systemd/nginx config; deploy = build_web + sync.sh.
What: server.py serves the systemd-inherited socket (LISTEN_FDS, fd 3) when offered, and treats unix-socket peers as the trusted proxy (X-Forwarded-For). build_web.py copies server.py into target/web/lobby/. Host side (outside this repo, ~/files/projects/web/nix/pyapp.nix, imported by flake.nix): pyapp@ socket/service/path template units, one nginx regex location /<app>/_srv/ -> /run/pyapp/<app>.sock, `pyapp.apps` list. Verified: 6 server tests pass; real unix-socket upgrade with inherited fd 3; nix module evaluates (units + nginx location). Not deployed or tested on the host.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 06:30 PM EST — web default server, .py no longer blocked
network.rs (wasm): default server is `<location.host><page dir>_srv`; `?noserver` disables, `?server=` overrides. Removed pyapp.nix's `.py`/`.sqlite` 404 rule (state is outside /srv; source should be downloadable). DEPLOY.CODEX.md updated. wasm check compiles; not run in a browser.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 06:45 PM EST — runtime app discovery
Removed the `pyapp.apps` list from web/nix/pyapp.nix: the app set isn't known at config time. New pyapp-scan.service + 15s timer (script web/nix/pyapp-scan.sh) starts pyapp@<dir>.socket/.path for each /srv/<dir>/server.py and stops units whose file is gone. Scanner verified with a mock systemctl (valid, invalid-name, no-server.py, vanished cases); module evaluates. Not deployed.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 07:00 PM EST — app logs endpoint
web/nix/pyapp.nix: journald gateway on 127.0.0.1:19531 (socket rebound from all interfaces), nginx `/<app>/_logs` (basic auth via /etc/pyapp.htpasswd, fixed unit filter, last 200 lines, `?follow` streams), PYTHONUNBUFFERED=1 on pyapp@. Module evaluates; gatewayd socket, proxy URL and maps checked in the evaluated output. Not deployed, so gatewayd's Range handling and the live stream are untested.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 07:15 PM EST — X-Forwarded-For spoofing fix
pyapp.nix `_srv` location: disabled recommendedProxySettings and send `X-Forwarded-For $remote_addr`; the stock `$proxy_add_x_forwarded_for` appends to a client-supplied header and server.py trusts the first entry, so clients could choose their own IP/town. Module evaluates; not deployed.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 07:40 PM EST — watchdog and SIGTERM save
server.py: sd_notify `WATCHDOG=1` from the tick loop (only when WATCHDOG_USEC is set); SIGTERM now cancels main so the final save runs. Fixed a latent shutdown hang: `async with listener`/`serve_forever()` await wait_closed(), which blocks until every client disconnects (so a stop with players connected would have timed out and skipped the save); main now saves, then `listening.close()` without waiting. pyapp.nix: WatchdogSec=30, NotifyAccess=main. 4 new tests (ping, no ping without watchdog, blocked loop stops pings, SIGTERM saves + exits 0 with a client connected); all server tests pass. Not run under real systemd.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 08:00 PM EST — server connection icon in the web menu
network.rs (wasm): `publish_status` emits a `corn-lobby-network` event (state off/connecting/joining/connected/stalled/replaced, url, room, id, players, reconnects, message) when it changes; Runtime gained last_message/closes ("stalled" = open socket silent >3s). web/: wifi-style icon on the menu's top line (green/amber/red/grey), hover or keyboard focus shows server, room, id, players, connected-for, reconnects, message. Wasm and native test builds compile; icon checked in headless Chromium with a mocked event; not yet against a live server.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 08:20 PM EST — surface connection errors
network.rs: a `transport_closed` carrying a failure reason now logs `warn!("Connection error: …")` once per distinct error (Runtime.last_error, cleared on welcome) and sets NetworkBridge.status to "Connection error: …" (shown in the web icon's Message row; cleared on welcome). Motivation: native's ws://-only transport silently retried a wss:// URL forever. Native and wasm builds compile; not run against a failing connect.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 08:30 PM EST — log connection success and player count
network.rs: first snapshot after a welcome logs `info!("Connected to <url>, room <room>, as <id>: N player(s) in the room")`, later count changes log `Players in room <room>: N` (Runtime.logged_players, reset on welcome/close). Native and wasm builds compile; not run against a live server.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 09:00 PM EST — native wss://
demo/lobby/src/network/websocket.rs: `address()` accepts wss:// (443 default, returns TLS server name); `tls_stream()` wraps the socket with rustls (ring provider named explicitly because aws-lc-rs is also linked; system roots via rustls-native-certs; handshake retried past the 50 ms read timeout). run() is generic over a boxed Read+Write. Cargo.toml: rustls + rustls-native-certs for non-wasm (both already in the lock). Unit tests cover ws/wss/IPv6 addresses; ignored live test `wss_live` connected to wss://sonoran.xyz/lobby/_srv over TLS and received a welcome. Full native game not launched against it.

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 07:40 PM EST
Scope: lobby remote-player orb and town label (demo/lobby network visuals, server relay).
Why: smaller, unobtrusive labels that show through-wall state and carry each player's sticky-note typeface; a fainter orb.
What: label_size default 18→9; orb_radius 0.12→0.09 and orb_color alpha 0.2 (material uses AlphaMode::Blend). Labels dim to 50% alpha when a physics ray from the camera to the label anchor hits collision (UI has no depth test, so this is a per-frame raycast, not a depth buffer test). New `post_it::LastFont` resource tracks the font index of the note being edited; network.corn sends it as `font`, server.py validates (int 0..63) and relays it in player snapshots, visuals.rs builds a Font from the embedded FONTS bytes per index and applies it to the peer's label (missing/old-server value → Gelasio, index 0).
Verification: cargo check --tests, lobby network tests, and server tests pass. Not run live (no visual check).

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST
Scope: lobby remote-player orb and town-name label lag when looking around.
Why: `network/visuals.rs::update` ran in `Last`, after transform propagation and UI layout, so the label's screen position and the orb's GlobalTransform applied one frame late.
What: `update` now runs in `PostUpdate` after `TransformSystems::Propagate` and `CameraUpdateSystems`, before `UiSystems::Prepare` (same ordering as zoom.rs); the orb also gets its `GlobalTransform` set directly. TODO(audit) comments mark this as a workaround to reevaluate (GlobalTransform one-frame delay, here and in general). Builds; network tests pass; not run visually.

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST
Scope: lobby network server-reset handling (network.corn, network.rs).
Why: a server reset restored doors/props but left players where they were, so a re-closed door could lock them outside.
What: setup captures the player's scene-load pose (`capture_spawn`); `reset(true)` now fires `reset_player`, which teleports the player to that spawn (no obstruction test) on a server epoch change and on a reconnect welcome. A saved-pose restore from the welcome still applies afterward and keeps its own obstruction fallback. Split `place_player` out of `restore_player`. Compiles; not run live.

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST
Scope: lobby server (demo/lobby/server/server.py) daily reset.
Why: sticky notes persisted forever; wanted a daily clean slate without disturbing players.
What: `daily_reset` wipes notes, objects and owners (placed radio kept, as in `expire_empty`) and issues a new epoch once per day after 4am America/New_York, only while the room has no connected players; a busy room waits until empty. Last handled boundary is stored per room (`daily:<room>`), so restarts neither repeat nor skip it; first run only starts the schedule. Runs on room load and every tick. Clients then reset and move players to spawn via the epoch change. New `wall_clock` ctor param; `reset_objects` factored out of `expire_empty`. Added a unit test; all 11 server tests pass.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST — daily reset is now total
The 4am reset also clears the radio placement and deletes the room's saved player records (identity, pose) via new `Store.delete_prefix`; returning players get a fresh identity and spawn. Test extended; 11 server tests pass.

# CLAUDE: claude-sonnet-5-5 → claude-opus-5-5 low — Fri Nov 06 2026 07:55 PM EST
Scope: lobby computer Reset (ui.rs, network.rs, network.corn, lobby.corn, server.py) + tests, NETWORK.CODEX.md.
Why: let a player reset the whole lobby for everyone from the computer.
What: desktop gets a Reset button above Start and a full-window Yes/No dialog (blocks clicks beneath; mirrored to observers). Yes, only from the player actually at the computer, triggers `ResetLobby`; network.rs sets `NetworkBridge.reset_requested` (offline: status says a server is needed). network.corn sends `reset: true` on updates until a new epoch answers. Server `reset_all`: new epoch + snapshot `reset` id, objects/owners cleared (radio placement too), every player pose (live and stored, via `Store.clear_poses`) forgotten, each note survives with NOTE_SURVIVAL=0.1; RESET_COOLDOWN 3s. Clients seeing the id change: `reset(false)` for door/keys/radio/computer, `resetProps` for local flashlights (dropped, off, authored pose), lobby.corn `reset_notes` (carried paper despawned, stacks restored, note coroutine bails via `noteGeneration`), own non-surviving notes despawned, then `scatter_player`: first of 32 random cabin spots the capsule fits (spawn fallback). Outgoing is skipped on the reset frame (stale sample). `player_obstructed` factored out of `restore_player`. Voice box intentionally untouched. Fixed stale `spawn` field in network tests. New: 2 server tests, 1 Rust test.

# CLAUDE: claude-opus-5-5 → claude-sonnet-5-5 low — Fri Nov 06 2026 08:14 PM EST
Scope: lobby computer Start readout (demo/lobby/src/ui.rs).
Why: show lobby state on the monitor when Start is clicked.
What: Start now lists the server address (or N/A when offline), the count of posted sticky notes (with how many are remote), the door state (open/closed, unlocked/locked) and radio on/off, after "Demo started.". Network resources are optional so the desktop still works without the network plugin. Builds; the first version was checked live (offline and against a scratch server), the final address-only version was built but not run, at the user's request.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST — live two-client test fixes
Ran the reset live with two native clients (temp server, NoMouse) and fixed two bugs it exposed: `wipe` read `cabin_min.x` but `get_resource` gives Vec3 as arrays, so every teleport candidate was NaN and players fell back to spawn (now `vec(...)`); a mirrored observer's Reset dialog stayed open after the reset (new `ui::close_reset_dialog`, called by `scatter_player`). Re-run passes: No keeps everything; Yes gives a new epoch on both clients, door closed, key loose, radio default, flashlight dropped and off, 30 notes -> 2, players at different random cabin spots, dialog closed. Also fixed the post-it editor test's missing `LastFont`.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST — radio "inside" check
`outgoing()` read `settings.cabin_min.x`, but a resource's nested Vec3 reaches scripts as an array (only a whole Vec3 value has .x), so `inside` was always false and the server never kept the radio's placement across resets. Now `vec(...)` + index. Checked live: a raw spectator socket sees `inside: true` once the radio settles in the cabin. Audited the other .corn scripts for the same pattern: none (lobby.corn's `t.x` is on a whole Transform value, which is fine; network.corn's `pose()` already used `vec`).

# CLAUDE: claude-opus-5-5 medium — Fri Nov 06 2026 10:45 PM EST
Scope: lobby interaction sounds (new foley.rs, tools/make_interaction_audio.py, assets/sounds/interact/, lobby.corn, radio.corn, network.corn, radio.rs, computer.rs, main.rs).
Why: no feedback sound for picking up/hanging the big key, taking/sticking post-its, drops, the flashlight and radio switches or the computer's mouse; a remote player's unlock was silent.
What: 9 generated mono Ogg one-shots (key pickup/hang, note peel/stick, flashlight on/off, radio click, mouse down/up). `SwitchSound {on, off, gain}` + script-triggerable `Switched {entity, on}` event (flashlight via toggle_flashlight, radio via radio.rs, the Monitor's for mouse press/release at the physical mouse). Drops: every prop's existing `KickSound` now also plays when a dynamic prop touching something loses >= 0.3 m/s in one physics step (`ImpactSettings`, volume 40-100% scaled to 2.5 m/s); not "contact started", since R's speculative contact starts at rest. Key/note sounds are lobby.corn children of the item (`sound_on`). Remote (spatial): network.corn plays unlock (new) and the door creak (now at the handle, 0.9, as locally) via lobby.corn's exported `sound`/`at_handle`, the radio click on a remote on/off change, and note_stick for notes posted after joining. User chose: generated sounds; remote for unlock/creak, radio and notes (not key pickup/hang); no small-key pickup sound.
Verified: 4 new foley tests, all 73 lobby tests pass. Live (native, CORN_DEV, temp server + a stdlib fake second player): every sound fires once on the right entity; remote unlock/radio/note/open each heard once; no echo click for local radio toggles. Not listened to by a human; web build not run.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 PM EST — forget positions on server restart
A restarted server restored players to their old saved position. Player records now carry a per-process `boot` id (`Server.record`); `join` drops the saved pose of a known player whose record is from another process (legacy records without `boot` count too), so returning players spawn fresh with the same identity. A reconnect to the same process still restores the pose. Server test updated; 13 server tests and the network Rust tests pass. Not run live.

## CLAUDE: claude-opus-5-5 medium — Fri Nov 06 2026 11:20 PM EST — "sound effects are temporary" hint
New reflected `SoundHint { sounds, text, shown }` resource (foley.rs): the first time this session any listed sound starts (observer on `Add<AudioPlayer>`, matched by asset path, so script- and Rust-spawned sounds alike), it triggers `SayCentered` "sound effects are temporary" once. Default list: big key pickup/hang/drop (kicks/large_key.ogg), flashlight on/off/drop (kicks/flashlight.ogg). New test (listed vs unlisted, once); 74 lobby tests pass; checked live by picking up the big key (screenshot).

# CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 10:58:00 PM EST
Scope: web lobby render resolution setting (launcher card, launcher.css, browser.rs).
Why: render fewer pixels on high-DPI displays; live switching, and no render-to-image.
What: a Full / 2/3 / 1/2 select on the launcher card (also the Escape menu), saved in localStorage and applied live. Fractions are of the display's physical pixels (1 = every device pixel). winit sizes the buffer from the canvas CSS box times devicePixelRatio, so launcher.css shrinks the box to the fraction and stretches it back with a transform, keyed on the canvas's data-render-scale (set by launcher.js); the buffer shrinks through the normal resize path. The rules are !important stylesheet rules because winit writes inline width/height at startup. browser.rs render_scale sets the window's scale factor override to devicePixelRatio * scale, so the logical size, and all UI, is the same as at Full. Window scale_factor_override alone and setting canvas width/height directly don't change the buffer (override: unchanged; direct: cropped). Native is unaffected.

## CLAUDE: claude-sonnet-5-5 low — Fri Nov 06 2026 — implementation
Launcher select + inline canvas styles; an initial HudScale divide-by-scale compensation made the HUD 1/f too big and was removed.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 — startup and fixed-size UI fixes
Starting at a saved 1/2 was zoomed/offset: winit's inline width/height replaced the script's, leaving only the transform. Moved the sizing to !important stylesheet rules. Fixed-size UI (player name labels) grew by 1/scale: added the scale factor override. Verified in Brave through devtools (dpr 1.667): startup at 1/2 and live Full/2/3/1/2 give buffers 3360x1930 / 2240x1287 / 1680x965 with a constant 2016x1158 logical window; the view is centered. HUD not checked in-game (needs mouse capture).

## CLAUDE: claude-opus-5-5 medium — Sat Nov 07 2026 05:47 PM EST — web remote console
corn_scripting remote_console now builds on web: `corn(code, generator?)` on the page (promise of the value / the
error) feeds the same per-frame request path as the native socket; debug builds, or `?corn_dev` in release. Screenshots
stay native. glue.js repl defines host api names the global object already has (window.find) instead of skipping them;
js_web describe() keeps a thrown string as is. Lobby: NoMouse counts as captured on web (browser.rs), so scripted input
works without pointer lock; smoke.js shot() is a no-op without screenshot(). Verified in Brave via chrome-devtools MCP
(dev build, own port/server): find/vars/generators/errors, smoke take-key + unlock, door open (creak on the frame the
animation starts). Native corn_scripting tests pass (21). Note: dooropen's first 1.12 s are still (key turn?).

# CLAUDE: claude-opus-5-5 low — Sat Nov 7 05:54:58 PM EST 2026
Scope: texture_loading loader; lobby radio station-list texture.
Why: Firefox web build quit at texture load. 16-bit PNGs (CODEX_radio_dial/regions_256) decode to Rgba16Unorm, which
WebGL2 only allows with EXT_texture_norm16 (Chromium yes, Firefox no); the invalid texture is a fatal render error.
What: loader.rs `to_8_bit` narrows R/Rg/Rgba16Unorm to 8-bit (Rgba8UnormSrgb when glTF says sRGB) before mips, and
warns with the path asking for an 8-bit re-save. Converted CODEX_radio_stations_256.png (art/, demo/lobby/assets) to
8-bit; dial and regions left 16-bit because art/lobby.blend packs them.

## CLAUDE: claude-opus-5-5 low — Sat Nov 7 05:54:58 PM EST 2026 — 16-bit texture fix
cargo check -p texture_loading passes; user confirmed the rebuilt web lobby (build_web.py) works in Firefox. The two radio textures now sample as sRGB (before: linear
Rgba16Unorm in Chromium), so they may look darker than in previous Brave runs. Stations PSNR vs 16-bit original 76 dB.

## CLAUDE: claude-opus-5-5 medium — Sat Nov 07 2026 05:57 PM EST — creak delay
lobby.corn door_opened is a generator: waits DOOR_CREAK_DELAY (1.08 s, where dooropen's still lead-in ends; doorcreak.ogg
has no leading silence) before the creak. Native check: the door starts moving ~1.08 s after opening; creak spawned at
1.14 s with the earlier 1.12 value.

# CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 06:24 PM EST
Scope: lobby cricket ambience volume (new demo/lobby/src/cricket_volume.rs, #sound in lobby.corn, main.rs registration).
Why: crickets should be louder by the windows and outside, and swell as the door opens. The user dropped the spatial
emitters + Ctrl-/ audio map idea for now ("not worth doing this way, right now").
What: `CricketVolume` on #sound (all tuning in lobby.corn) sets its AudioSink volume each frame from the listener's
[x, z]: 0.2 inside, 0.35 outside the cabin footprint; linear ramps of 1.5 m from each window's center and 1.5 m
centered on the door's plane, whose inner edge moves 1 m further in over 3 s (dooropen's length) while the door is open.
Window centers measured by ray-scanning the shell's openings: back (-0.05, -4.0), left (-2.3, -1.65), right (2.3, 1.7).

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 06:24 PM EST — cricket volume by windows and door
Native build passes; live run read the parsed component back and a screenshot confirmed the right window. Unit test
added but not run: the lobby test build fails on network/tests.rs (missing `voice_error`, unrelated in-progress work).
Not listened to.

# CLAUDE: claude-opus-5-5 low — Sat Nov  7 06:25:28 PM EST 2026
Scope: lobby remote player (town) labels.
Why: labels should read as world-space objects, shrinking with distance, but stay legible far away.
What: NetworkSettings.label_size replaced by label_world_size (font em height in metres, default 0.04) and label_min_size (logical px floor, default 12). visuals.rs projects a label_world_size segment along camera-up at the label anchor to get its on-screen height (so FOV and right-click zoom apply), floors it at label_min_size and rounds to whole px so the glyph atlas doesn't fill with fractional sizes.

## CLAUDE: claude-opus-5-5 low — Sat Nov  7 06:25:28 PM EST 2026 — implementation
Native lobby build passes. Lobby tests don't compile because of a pre-existing error (network/tests.rs Runtime missing voice_error), not this change. Not tested visually.

# CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 06:32:15 PM EST
Scope: lobby remote player orbs (demo/lobby/src/network/visuals.rs, network.rs) and network_audio speaker levels.
Why: make orbs look nicer and show who is talking.
What: original orb look kept (core+glow-shell version was tried and reverted at user request), now with a per-peer material. Idle orbs breathe (orb_pulse) and bob, phase-offset per peer. A speaking peer's orb swells (orb_speaking_swell, default 0.4) and gains opacity/emissive with their voice RMS (orb_speaking_rms = full), fast attack / slow release. network_audio gained VoiceClient::levels() (id -> pre-spatial RMS) on native (fresh within 150 ms) and web (AnalyserNode per remote speaker in voice.js); Runtime.voice_levels carries it to visuals.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 06:32:15 PM EST — implementation
Native + wasm32 check pass; lobby tests pass (75 + 10). Not tested visually.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 — revert orb look
Back to the single translucent emissive uv sphere; pulsing/bob and speaking swell+brightness kept on it. Tests pass; web release rebuilt.

# CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026
Scope: lobby remote player orbs (demo/lobby/src/network/visuals.rs).
Why: tell players apart by orb colour.
What: each peer's orb hue is derived from its server id (FNV-1a hash -> 0..360 degrees), applied over NetworkSettings.orb_color's saturation/lightness/alpha, so orb_color still tunes brightness and opacity live. Ids persist per player token on the server, so a player keeps their hue across reconnects with nothing extra stored. Hash collisions are possible (two players can land on similar hues).

## CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 — implementation
Two unit tests added (stable/in-range/distinct hue; tint changes only hue) and pass. Not tested visually or with live peers.

# CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 07:20 PM EST
Scope: lobby networking startup (demo/lobby/src/network.rs, main.rs).
Why: a mistyped LOBBY_SERVER (127.0.0.8799) left the game offline and silently blocked using the computer.
What: removed gate_computer (E on the computer always works). Native: a malformed LOBBY_SERVER (bad host/IPv4/port) panics at startup; a valid but unreachable server logs an error and exits with code 1 (main returns AppExit). Web unchanged.

## CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 07:20 PM EST — verification
Native build passes; bad address panics, unreachable address exits 1, 127.0.0.1:8799 connects. Lobby tests not run (pre-existing compile error in network/tests.rs).

# CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 07:48 PM EST
Scope: art/lobby.blend, live session only (not saved): black-and-white radio region map on barrel_freestanding's lid.
Why: user asked for a monochrome copy of the radio map drawn on the barrel, using the sticky-note technique.
What: added CLAUDE_barrel_map_bw, a 0.17 m world-aligned quad (4x4 grid) parented to barrel_freestanding, in the same collection as the lid arrow, offset 0.2 m toward -Y from the barrel centre and 0.0095 m above the lid like the arrow. Own UV map `CODEX_overlay_uv`, packed 256px texture (art/CLAUDE_barrel_map_bw_256.png) wired directly to a BLENDED copy of postit_handwritten_ink. Map is the same ground-aligned data as the desk note, monochrome: pink regions and the cabin black, the upper half of "numbers" white with a black outline, radio X black, on opaque white.
No Shrinkwrap on the quad: the lid is flat and the barrel is a collection instance whose source meshes sit elsewhere, so a shrinkwrap would do nothing. Checked in a temporary-camera EEVEE render (camera removed, scene camera restored). No glTF export or game run.

## CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 07:48 PM EST — scripts
.agents/tmp/barrel_map_bw_svg_CLAUDE.py (SVG generator, derived from map_texture_CODEX.py) and .agents/tmp/barrel_map_overlay_CLAUDE.py (builds the mesh/material).

# CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 07:52 PM EST
Scope: lobby radio, network.corn, lobby server.
Why: #radio_near_light should only come on once the radio has stayed tuned to 0.9 (±0.038) for 5 consecutive seconds (in tuning mode or not), shared by all players and reset with the radio.
What: Radio gains networked `near_light`, latched by near_light_unlock (each client times the networked dial). New `RadioNearLight { frequency, tolerance, seconds }` (0.9/0.038/5) on #radio_near_light in lobby.corn; its Visibility follows any radio's `near_light`. network.corn sends/applies/baselines it with on/tuning (`radioFields`), so local reset() clears it; the server drops it on empty/computer/daily resets and on durable restore. Server test asserts near_light is cleared by the empty reset.
Note: lobby.gltf has no node named radio_near_light yet (only "radio light", "radio light.001").

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 07:52 PM EST — implementation
22 lobby radio tests pass; server tests pass except an unrelated geo-name test (test_geo_local_cache_and_failure: 'localhost' != 'Local'). Not run in-game.

## CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 08:00 PM EST — transparent map with white X
User moved CLAUDE_barrel_map_bw onto barrel_near_region (scale ~3.09, collection `barrels`) and kept that transform. Regenerated its texture: transparent background (no white paper) and a white X centred on the near circle (radio_signal_near centre, same ground mapping). The upper "numbers" half stays white ink and the radio-start X stays black. Replaced art/CLAUDE_barrel_map_bw_256.png, reloaded and repacked the Blender image (alpha 0 at corners), verified with a temporary-camera render (camera removed, scene camera restored). Generator .agents/tmp/barrel_map_bw_svg_CLAUDE.py updated. lobby.blend not saved.

## CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 08:10 PM EST — no white anywhere on the barrel map
The near-circle X is now a true cut-out (transparent, lid wood shows through) and the upper half of the "numbers" disk is unfilled with a black outline arc. The texture is pure black on transparent (verified: max RGB of visible pixels = 0). Replaced art/CLAUDE_barrel_map_bw_256.png and repacked the Blender image; generator now writes a second X-cut SVG that is subtracted with ImageMagick DstOut. lobby.blend not saved.

## CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 08:20 PM EST — red smudge replaces radio-start X
Removed the black radio-start X from the barrel map and painted a blurred, slightly translucent red smudge (#b3191a, drag tail pointing away from the cabin) at the same radio_spawn-derived position. Texture is otherwise unchanged (black on transparent, clear X cut-out, clear upper "numbers" half). Replaced art/CLAUDE_barrel_map_bw_256.png and repacked the Blender image. Generator writes the smudge position to OUT.radio.txt; .agents/tmp/barrel_map_smudge_CLAUDE.sh paints it. lobby.blend not saved.

# CLAUDE: claude-sonnet-5-5 low — Sat Nov 07 2026 08:30 PM EST
Scope: demo/lobby/assets/lobby.corn, collider rule for barrel_near_region.
Why: user asked for the near barrel to use the same collider setup as the radio-start barrel.
What: added #barrel_near_region and #barrel_near_region #Cylinder to the existing `#tabletop, #barrel_freestanding, #barrel_freestanding #Cylinder` rule (Body: Static, TrimeshFromMesh), so the near barrel's nested Cylinder is no longer caught by the shared `#Cylinder` convex-hull rule. Not built or run; no in-game check of dropped props settling on it.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:20 PM EST — undid an accidental `cargo fmt -p lobby`
It reformatted ~33 lobby files. Safety stash: `stash@{0}` "CLAUDE: before reverting cargo fmt" (tracked files only). Restored tracked files from HEAD, main.rs as HEAD + its uncommitted cricket_volume/AppExit edits, and the 8 reformatted untracked files (cricket_volume, door, foley, network, network/{tests,visuals,websocket}, zoom) from VS Code/Claude file history, a git blob and a transcript replay. Each restore was checked as rustfmt(restored) == the reformatted file byte for byte; 77+10 lobby tests pass.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:35 PM EST — simpler rule
Dropped the pickup requirement and `picked_up`; light now latches after 5 s in a row within 0.038 of 0.9, no need to leave tuning. Unit test near_light_needs_five_consecutive_seconds_in_band; 78+10 lobby tests pass.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:40 PM EST — TODO
TODO above near_light_unlock (radio.rs): networking should give one authority the latch instead of every client claiming the radio. Done for now.

# CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:17 PM EST
Scope: demo/lobby/assets/lobby.corn, use_door. Also a read-only diagnosis of the server pose-restore bug (no changes).
Why: STOPGAP for players stranded outside a locked door (e.g. a pose restored after the server's empty-room reset_objects, which re-locks the door but keeps saved poses).
What: with the door locked and the player past the door's plane (+z, `outside()`), clicking the door puts the realkey into the key_slot from the outside (KEY_IN_SLOT_OUTSIDE: half-turned about y, bow on the outside face) wherever it is, unless another player carries it (RemoteHeld), and unlocks; the next click opens as usual. Carrying the realkey from outside also uses the outside pose. Slotting moved into slot_key(). Syntax checked with node; not run in game.

# CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:18:58 PM EST
Scope: lobby held-item placement (flashlight).
Why: the held flashlight took too much screen when looking up or down.
What: new `Holdable.pitch_below`, extra screen-height share below centre per radian of |camera pitch|, linear, 0 at level view (default 0 so other props are unchanged). Flashlight set to 0.1 in lobby.corn. Applies to both carry and drop poses. Also `Holdable.aim_pitch_up` (optional): beam pitch multiplier when looking up; flashlight 1.0 so the beam follows the view upward, down keeps aim_pitch 0.6. `pitch_below_up` (optional) tunes the hold height separately for looking up; unset falls back to pitch_below.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:18:58 PM EST — pitch-dependent hold height
Added the field and wired it into held_pose; hold tests pass.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:22:25 PM EST — upward beam follows view
Added aim_pitch_up (None = aim_pitch); flashlight 1.0. Hold tests pass.

## CLAUDE: claude-opus-5-5 low — Sat Nov 07 2026 08:27:36 PM EST — separate up/down hold height
Added pitch_below_up (None = pitch_below); flashlight values left as the user tuned them.
