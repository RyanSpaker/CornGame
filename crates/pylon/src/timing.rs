//! CLAUDE (claude-opus-5-5): 2026-11-02 `PYLON_TIMING=1`: how long loading takes and what scripts cost a
//! frame, against the same per-frame work done in rust. Logs, then exits:
//! - load milestones from startup: the scene file loaded (built), spawned, the first steady frame
//! - 300 frames outside, then 300 inside the woken room: `drive_scripts` per frame (all script work) and
//!   its host calls by kind (corn_scripting's `profile` feature), and [`rust_effects`] + [`rust_chirps`],
//!   rust copies of pylon.corn's `interior_effects` and `chase_chirps` writing the same values
//!
//! - GPU time per frame by render pass (bevy's render diagnostics: timestamp queries), summed over the
//!   views that ran the pass, and the window's size (GPU time scales with it)
//!
//! corn_scene logs each file's build stages at debug (`RUST_LOG` is set for it in main.rs).

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use bevy::{
    diagnostic::DiagnosticsStore,
    prelude::*,
    render::diagnostic::RenderDiagnosticsPlugin,
    window::PrimaryWindow,
    world_serialization::WorldInstanceReady,
};
use pylon::{door::DoorState, interpolate::Target, player::Player, portal::InRealm};

pub fn plugin(app: &mut App) {
    app.add_plugins(RenderDiagnosticsPlugin)
        .insert_resource(Timing::default())
        .add_observer(|_: On<WorldInstanceReady>, mut t: ResMut<Timing>| t.mark("scene spawned"))
        // in fixed steps, as the script runs: the same values from the same clock. before the script's
        // FixedUpdate, so it finds the crystals where they were spawned
        .add_systems(FixedPreUpdate, rust_effects)
        .add_systems(Update, rust_chirps)
        .add_systems(Last, measure);
}

#[derive(Resource)]
struct Timing {
    start: Instant,
    marks: Vec<(&'static str, Duration)>,
    phase: Phase,
    frames: u32,
    script: Vec<Duration>,
    rust: Vec<Duration>,
    frame: Vec<Duration>,
    host: Vec<(&'static str, corn_scripting::profile::Spent)>,
    /// rust_effects' (0, 1 or more fixed steps) and rust_chirps' time this frame
    rust_now: Duration,
    /// crystals checked against what the script left, and how many disagreed
    checks: (u32, u32),
    /// GPU ms by pass (`render/<pass..>`), summed over the measured frames; by the order the views ran
    /// it in (cameras by `order`, the player's last)
    gpu: BTreeMap<String, Vec<f64>>,
    /// the newest render diagnostics already counted
    gpu_seen: Option<Instant>,
    window: UVec2,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            marks: vec![],
            phase: Phase::Loading,
            frames: 0,
            script: vec![],
            rust: vec![],
            frame: vec![],
            host: vec![],
            rust_now: Duration::ZERO,
            checks: (0, 0),
            gpu: BTreeMap::new(),
            gpu_seen: None,
            window: UVec2::ZERO,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum Phase {
    Loading,
    Settling,
    Outside,
    Entering,
    Inside,
}

impl Timing {
    fn mark(&mut self, what: &'static str) {
        self.marks.push((what, self.start.elapsed()));
    }

    fn report(&mut self, label: &str, scene: &str) {
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let stats = |v: &mut Vec<Duration>| {
            v.sort();
            let mean = v.iter().sum::<Duration>() / v.len().max(1) as u32;
            format!("mean {:.3}ms, median {:.3}ms, p99 {:.3}ms", ms(mean), ms(v[v.len() / 2]), ms(v[v.len() * 99 / 100]))
        };
        info!("timing {label}: {} frames; {scene}", self.script.len());
        info!("timing {label}: frame        {}", stats(&mut self.frame));
        info!("timing {label}: scripts      {}", stats(&mut self.script));
        info!("timing {label}: rust copy    {}", stats(&mut self.rust));
        let n = self.script.len().max(1) as f64;
        let host: Vec<String> = self
            .host
            .iter()
            .filter(|(kind, _)| *kind != "drive_scripts")
            .map(|(kind, s)| format!("{kind} {:.1}/frame {:.4}ms/frame", s.calls as f64 / n, ms(s.time) / n))
            .collect();
        info!("timing {label}: host calls   {}", host.join(", "));
        info!("timing {label}: rust copy and script disagreed on {} of {} crystal targets", self.checks.1, self.checks.0);
        self.checks = (0, 0);
        // passes nest (`render/a/b`): the total is the outermost ones'
        let top: f64 = self.gpu.iter().filter(|(path, _)| !path.contains('/')).flat_map(|(_, t)| t).sum();
        info!("timing {label}: gpu          {:.3}ms/frame in passes, window {}x{}", top / n, self.window.x, self.window.y);
        let mut passes: Vec<_> = std::mem::take(&mut self.gpu).into_iter().map(|(path, t)| (t.iter().sum::<f64>(), path, t)).collect();
        passes.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (total, path, views) in passes.iter().filter(|(total, ..)| total / n >= 0.01) {
            let views: Vec<String> = views.iter().map(|t| format!("{:.3}", t / n)).collect();
            info!("timing {label}: gpu   {:>8.3}ms  {path}  (by view: {})", total / n, views.join(" "));
        }
        self.script.clear();
        self.rust.clear();
        self.frame.clear();
        self.host.clear();
    }
}

fn measure(
    mut t: ResMut<Timing>,
    time: Res<Time>,
    roots: Query<&bevy::world_serialization::WorldAssetRoot>,
    assets: Res<AssetServer>,
    mut players: Query<(&mut Transform, &mut InRealm), With<Player>>,
    mut exit: MessageWriter<AppExit>,
    diagnostics: Res<DiagnosticsStore>,
    window: Single<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, Option<&Name>)>,
    doors: Query<&DoorState>,
) {
    let spent = corn_scripting::profile::take();
    // what was being drawn, for the report
    let rendering: Vec<&str> = cameras.iter().filter(|(c, _)| c.is_active).map(|(_, name)| name.map_or("?", |n| n.as_str())).collect();
    let doors: Vec<String> = doors.iter().map(|d| format!("{:?}", d.state)).collect();
    let scene = format!("cameras rendering: {}; doors: {}", rendering.join(", "), doors.join(", "));
    let frame = Duration::from_secs_f64(time.delta_secs_f64());
    // the render diagnostics that arrived since last frame (each pass once per view that ran it)
    let measuring = matches!(t.phase, Phase::Outside | Phase::Inside);
    let seen = t.gpu_seen;
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().as_str();
        let Some(pass) = path.strip_prefix("render/").and_then(|p| p.strip_suffix("/elapsed_gpu")) else { continue };
        // one batch a frame; within it, one measurement per view that ran the pass
        let mut view = 0;
        let mut batch = None;
        for m in diagnostic.measurements().filter(|m| seen.is_none_or(|s| m.time > s)) {
            if batch != Some(m.time) {
                (batch, view) = (Some(m.time), 0);
            }
            t.gpu_seen = t.gpu_seen.max(Some(m.time));
            if measuring {
                let views = t.gpu.entry(pass.to_string()).or_default();
                if views.len() <= view {
                    views.resize(view + 1, 0.0);
                }
                views[view] += m.value;
            }
            view += 1;
        }
    }
    t.window = window.physical_size();
    match t.phase {
        Phase::Loading => {
            if !t.marks.iter().any(|m| m.0 == "scene file loaded") && roots.iter().any(|r| assets.is_loaded_with_dependencies(&r.0)) {
                t.mark("scene file loaded");
            }
            if t.marks.iter().any(|m| m.0 == "scene spawned") {
                t.phase = Phase::Settling;
                t.frames = 0;
            }
        }
        // until frames are steady (pipelines compiled, images uploaded): 30 in a row under 50ms
        Phase::Settling => {
            t.frames = if frame < Duration::from_millis(50) { t.frames + 1 } else { 0 };
            if t.frames == 30 {
                t.mark("steady frames");
                for (what, at) in t.marks.clone() {
                    info!("timing load: {what} at {:.0}ms", at.as_secs_f64() * 1e3);
                }
                t.phase = Phase::Outside;
                t.frames = 0;
            }
        }
        Phase::Outside | Phase::Inside => {
            let script = spent.iter().find(|s| s.0 == "drive_scripts").map_or(Duration::ZERO, |s| s.1.time);
            let rust = std::mem::take(&mut t.rust_now);
            t.script.push(script);
            t.rust.push(rust);
            t.frame.push(frame);
            for (kind, s) in spent {
                match t.host.iter_mut().find(|h| h.0 == kind) {
                    Some(h) => {
                        h.1.calls += s.calls;
                        h.1.time += s.time;
                    }
                    None => t.host.push((kind, s)),
                }
            }
            t.frames += 1;
            if t.frames == 300 && t.phase == Phase::Outside {
                t.report("outside", &scene);
                // into the room, away from the door: it wakes
                if let Ok((mut transform, mut realm)) = players.single_mut() {
                    transform.translation = Vec3::new(120.0, 1.68, 9.5);
                    realm.0 = 1;
                }
                t.phase = Phase::Entering;
                t.frames = 0;
            } else if t.frames == 300 {
                t.report("inside", &scene);
                exit.write(AppExit::Success);
            }
        }
        Phase::Entering => {
            t.frames += 1;
            if t.frames == 240 {
                t.phase = Phase::Inside;
                t.frames = 0;
            }
        }
    }
}

/// pylon.corn's `interior_effects` per fixed step, in rust: the crystals turn and bob
fn rust_effects(
    mut t: ResMut<Timing>,
    time: Res<Time>,
    mut crystals: Local<Vec<(Entity, f32, f32, f32)>>,
    mut wrote: Local<Vec<Transform>>,
    names: Query<(&Name, &ChildOf)>,
    children: Query<&Children>,
    // the crystals' targets: writing their Transforms would teleport them (interpolate.rs)
    mut transforms: Query<&mut Target<Transform>>,
) {
    // the script ran the last step after this did: it should have left what this wrote (not timed)
    for (&(e, ..), wrote) in crystals.iter().zip(wrote.iter()) {
        let Ok(target) = transforms.get(e) else { continue };
        let near = target.0.translation.distance(wrote.translation) < 1e-4 && target.0.rotation.angle_between(wrote.rotation) < 1e-3;
        t.checks.0 += 1;
        t.checks.1 += u32::from(!near);
    }
    let begin = Instant::now();
    // pylon.corn's S
    let scale = 0.5;
    if crystals.is_empty() {
        // numbered as the script numbers them: in their parent's children
        let crystal = |e: Entity| names.get(e).is_ok_and(|(name, _)| name.as_str() == "crystal");
        let parent = names.iter().find(|(name, _)| name.as_str() == "crystal").map(|(_, of)| of.parent());
        let found: Vec<Entity> = match parent.and_then(|p| children.get(p).ok()) {
            Some(kids) => kids.iter().filter(|&e| crystal(e)).collect(),
            None => vec![],
        };
        *crystals = found
            .iter()
            .enumerate()
            .filter_map(|(i, &e)| {
                let p = transforms.get(e).ok()?.0.translation;
                Some((e, i as f32, p.y, p.x.atan2(p.z) + i as f32))
            })
            .collect();
    }
    let now = time.elapsed_secs_wrapped();
    for &(e, i, y, phase) in crystals.iter() {
        if let Ok(mut target) = transforms.get_mut(e) {
            let tr = &mut target.0;
            tr.rotation = Quat::from_rotation_y(now * (0.42 + i * 0.015) + phase);
            tr.translation.y = y + (now * 1.7 + phase).sin() * 0.055 * scale;
        }
    }
    t.rust_now += begin.elapsed();
    *wrote = crystals.iter().filter_map(|&(e, ..)| transforms.get(e).ok().map(|t| t.0)).collect();
}

// CLAUDE
/// pylon.corn's `chase_chirps` ten times a second, in rust: inside, each chirp's next pass, and the ones due
/// soon set to play then (computed, not played)
fn rust_chirps(
    mut t: ResMut<Timing>,
    time: Res<Time>,
    mut tick: Local<f32>,
    mut at: Local<Vec<f64>>,
    players: Query<&InRealm, With<Player>>,
) {
    let now = time.elapsed_secs_wrapped();
    if now < *tick + 0.1 {
        return;
    }
    *tick = now;
    let begin = Instant::now();
    // pylon.corn's S, and chase_chirps' LEAD and AHEAD
    let (speed, lamps, passes, lead, ahead) = (0.2, 80usize, 1.0, 0.08, 0.25);
    let (t_now, period) = (now as f64, 1.0 / (speed * passes));
    at.resize(lamps, f64::NEG_INFINITY);
    if players.iter().next().map(|r| r.0) != Some(1) {
        at.fill(f64::NEG_INFINITY);
    } else {
        for (i, at) in at.iter_mut().enumerate() {
            let offset = i as f64 / (lamps as f64 * speed);
            let n = (((*at - offset) / period).floor() + 1.0).max(((t_now + lead - offset) / period).ceil());
            let next = offset + n * period;
            if next <= t_now + ahead {
                *at = std::hint::black_box(next);
            }
        }
    }
    t.rust_now += begin.elapsed();
}
