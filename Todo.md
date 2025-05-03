# TODO:
- World Inspector plugin needs to be re-added upon the release of bevy 13 (and world-inspector's update to 13).
- Corn Rendering Optimizations (there's so many options).
- Corn Rendering Improvements (Better Colors, Color Maps or different materials for each sub-mesh, Better Shadows, Make it night time, Flashlight).
- Create some sort of "debug mode" for the corn game which can set up entirely different rendering options, like full brightness isntead of night. In the future, having an easy way of switching to a debug view of the scene will be very important.
- Profiling Information: We need a huge amount of profiling data to make decisions about how to optimize the game.
- Gameplay elements: Better Sample Scene, Actual Character Controller, Main Menu (I really like phasmophobia's interactive main menu)

- integrate bevy_console and bevy_mod_scripting
- debug console pipe to server

- debug for resetting physics to zero and transform to initial

- add fromstr or parse to bevy Entity

# basic dev plan
- spray paint
- spy glass (scroll for depth of field / focus)
- gate key
- randomized map with wall, tower, and well
- ui menu and journal

# item idea vault
- radio ping (also picks up monsters)
- items have very limited battery
  - specific monsters can drain batteries faster

# bevy
add single method to children

# gotchas
- seems that registering two types with the same name doesn't log a warning(weird interplay with reflect).

# crashes

When gltf component has wrong type.
```
thread 'main' panicked at /home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/bevy_gltf_components-0.5.1/src/process_gltfs.rs:90:22:
Unable to reflect component
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Encountered a panic in exclusive system `bevy_gltf_components::process_gltfs::add_components_from_gltf_extras`!
Encountered a panic in system `bevy_app::main_schedule::Main::run_main`!
```

when enabling xpbd debug render
```
thread 'Compute Task Pool (12)' panicked at /home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/bevy_math-0.13.2/src/primitives/dim3.rs:40:9:
assertion failed: value.is_normalized()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Encountered a panic in system `bevy_xpbd_3d::plugins::debug::debug_render_axes`!
Encountered a panic in system `bevy_app::main_schedule::Main::run_main`!
```

# bugs

despawning sync'd car seems to create a bunch of entities with "replication" as only component, on the client.

somehow steping up onto or off the box can sometimes cause the box to go flying off the map. Some kind of bug in how character controller works.

bevy_console continually grows, probably a bug in that.

crazy client flickering of physics when networking
- seems that moving name_sync_test system to preupdate fixed this.
