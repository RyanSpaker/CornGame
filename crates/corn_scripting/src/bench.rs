//! CLAUDE (claude-sonnet-5-5): 2026-10-29 script engine benchmark (native).
//! Where does a frame's time go: host calls (live refs, get/insert, call overhead) or script compute?
//!
//!     cargo test -p corn_scripting --release bench -- --ignored --nocapture
//!
//! Every scenario is a js function run against 1000 entities with a Transform. Numbers from 2026-10-29
//! (QuickJS and Rune, since dropped, alongside) are in the scripting notes.
#![cfg(not(target_family = "wasm"))]

use std::time::Instant;

use super::*;

const ENTITIES: usize = 1000;
const FRAMES: usize = 200;

const JS: &str = r#"
// call overhead only
exports.noop = function (e, args, event) {};

// live refs: resolve + write through, one host round trip per field access
exports.live = function () {
  for (const e of find("Transform")) {
    const t = component(e, "Transform");
    t.translation.x += 1;
  }
};

// live refs, several accesses per entity
exports.live_many = function () {
  for (const e of find("Transform")) {
    const t = component(e, "Transform");
    t.translation.x += 1;
    t.translation.y += t.translation.x * 0.5;
    t.translation.z -= 0.25;
    t.scale.x = 1 + Math.sin(t.translation.x) * 0.1;
  }
};

// live_many with the child refs held: the most reusing child wrappers could save
exports.live_many_hoisted = function () {
  for (const e of find("Transform")) {
    const t = component(e, "Transform");
    const tr = t.translation;
    const s = t.scale;
    tr.x += 1;
    tr.y += tr.x * 0.5;
    tr.z -= 0.25;
    s.x = 1 + Math.sin(tr.x) * 0.1;
  }
};

// copies: get the component, modify, insert it back (math value, not JSON)
exports.copy = function () {
  for (const e of find("Transform")) {
    const t = get(e, "Transform");
    t.translation.x += 1;
    insert(e, "Transform", t);
  }
};

// json path: a non-math component through reflect serialization
exports.json = function () {
  for (const e of find("Transform")) {
    insert(e, "Bench", { n: get(e, "Bench").n + 1 });
  }
};

// no host calls at all: pure engine speed (~ a few million flops of objects and math)
exports.compute = function () {
  let sum = 0;
  const pts = [];
  for (let i = 0; i < 1000; i++) pts.push({ x: i, y: i * 2, z: i * 3 });
  for (let k = 0; k < 100; k++) {
    for (const p of pts) {
      sum += Math.sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
      p.x += 0.001;
    }
  }
  return sum;
};

// generators: 1000 threads, each steps and sleeps
exports.thread = function* (e) {
  while (true) {
    yield 0;
  }
};
"#;

#[derive(Debug, Default, Component, Reflect)]
#[reflect(Component, Default)]
struct Bench {
    n: f32,
}

fn world() -> World {
    let mut world = World::new();
    world.init_resource::<AppTypeRegistry>();
    let registry = world.resource::<AppTypeRegistry>();
    registry.write().register::<Transform>();
    registry.write().register::<Bench>();
    for i in 0..ENTITIES {
        world.spawn((Transform::from_xyz(i as f32, 0.0, 0.0), Bench::default()));
    }
    world
}

/// average ms per run of `f` over `FRAMES` frames, after a warmup
fn time(mut f: impl FnMut()) -> f64 {
    for _ in 0..10 {
        f();
    }
    let start = Instant::now();
    for _ in 0..FRAMES {
        f();
    }
    start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64
}

fn bench(engine: &dyn Engine) {
    let mut world = world();
    let entities: Vec<String> = world.query::<Entity>().iter(&world).map(entity_key).collect();
    with_world_scope(&mut world, || engine.load("bench", JS)).unwrap();
    let run = |world: &mut World, function: &str, target: Option<&str>| {
        with_world_scope(world, || engine.call("bench", function, 0, target, "{}", "null")).unwrap_or_else(|e| panic!("{function}: {e}"));
    };

    println!("\n== {ENTITIES} entities, avg ms per frame over {FRAMES} frames ==");

    // one call per entity: what the listener driver does for a per-entity event
    let ms = time(|| {
        for e in &entities {
            run(&mut world, "noop", Some(e));
        }
    });
    println!("{:<34}{ms:>9.3} ms  ({:.2} us/call)", "call overhead (x1000 calls)", ms * 1000.0 / ENTITIES as f64);

    for (label, function) in [
        ("live ref, 1 write/entity", "live"),
        ("live ref, 3 reads+4 writes/entity", "live_many"),
        ("  same, child refs held", "live_many_hoisted"),
        ("get + insert, math copy", "copy"),
        ("get + insert, json component", "json"),
        ("pure compute, no host calls", "compute"),
    ] {
        let ms = time(|| run(&mut world, function, None));
        println!("{label:<34}{ms:>9.3} ms");
    }

    // 1000 generators stepped once each per frame
    for thread in 0..ENTITIES as u32 {
        with_world_scope(&mut world, || engine.call("bench", "thread", thread + 1, None, "{}", "null")).unwrap();
    }
    let ms = time(|| {
        for thread in 0..ENTITIES as u32 {
            with_world_scope(&mut world, || engine.step(thread + 1, 0.016)).unwrap();
        }
    });
    println!("{:<34}{ms:>9.3} ms", "step 1000 generators");
}

#[test]
#[ignore = "benchmark: cargo test -p corn_scripting --release bench -- --ignored --nocapture"]
fn bench_engines() {
    if cfg!(debug_assertions) {
        println!("warning: debug build, run with --release");
    }
    bench(&js::Js::new().unwrap());
}

// CLAUDE
/// one scenario, for a profiler: BENCH_FN=live_many
#[test]
#[ignore = "profiling: BENCH_FN=live_many perf record -g <test binary> bench_profile --ignored"]
fn bench_profile() {
    let function = std::env::var("BENCH_FN").unwrap_or("live_many".into());
    let engine = js::Js::new().unwrap();
    let mut world = world();
    with_world_scope(&mut world, || engine.load("bench", JS)).unwrap();
    let start = Instant::now();
    for _ in 0..2000 {
        with_world_scope(&mut world, || engine.call("bench", &function, 0, None, "{}", "null")).unwrap();
    }
    println!("{function}: {:.3} ms/frame", start.elapsed().as_secs_f64() * 1000.0 / 2000.0);
}
