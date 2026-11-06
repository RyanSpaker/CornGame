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

## controller
- [ ] don't send data when player isn't moving
  - either round velocity or switch to networking inputs.
- [ ] why does game sometimes run really badly when two instances are open?
  - [ ] why does character physics break (oscillations) at low frame rates.
- [x] test client->server->otherclient replication for character
  - DONE: although testing was limited as I was getting 5fps per above issue.
  - [ ] test with hierarchy



# Tue Sep 15 08:00:45 PM EDT 2026
DEV PLAN V3

- prototype all features in three.js
  
  determine look/feel, needed tweaks and editor ui, good defaults, and prefered impl/variant before ever writing a line of rust

rust codebase
- remove lightyear and roll our own networking
  very little needs to be synced anyway

  events need individual logic most of the time
  - ex syncronizing a scripted set of transitions

  lightyear update to 0.19 removes ability to have client authority

  design
  - NetworkId for matching
    seperate spawn and sync which was an issue in replicon/lightyear
    networkid is normally a hash, but there is NetworkIdSrc which shows how it is computed so that it can be debugged. 
  - create seperate network sync entities which interpolate to real ones.
    make this easy to add later
  - scene spawner should work at many granulaties (sending "spawn scene X" or spawning scene X and streaming "spawn X object on Y parent in scene" as well as intermediate levels)
- rethink blender assets
  css-like syntax for attaching properities to gltf items based on 
    name, class, hierarchy
  executes at scene load, but can be changed in dev ui and reapplied
  used to attach colliders, cornfields, etc.
  consider BSN functionality overlap, though this is mostly for having a plaintext blender -> game layer. blender gets to use plain names
- we need a state machine + curves editor
- we need scripting, ideally in js
- we need a good terminal, more than any other part of the editor
- we need logging which is tied to entities (querying can use css selectors)
- we need StandardRetroMaterial and StandardBillboard

it would be nice to have shadergraph, and something like geometry nodes, but with a textual format in addition to visible. This is also the case for the statemachines.

We need way fewer libraries. We should disable all bevy features we don't use.

We need standard benchmarks.

We need to be able to build a windows dist

We need standardized values for lights, size, and materials so we can use physically sensisble defaults. 

We need some way to provide in blender a box of reference size (rough bounding box) which we can then scale to a desired size in code. Rather than magic numbers. Procedural geometry scripting where we can say "line up X.top with B.bottom" etc

Unsolved: settings.

we need a way to break large gltf scenes up into models and references
and this needs to be easily inspectible with cli tools

we need a way to convert blender -> gltf headless, makefile

we need a consistent representation for variable size textures and model LODs, so we can use the 4k in blender but game only ever loads the downscaled. 

we need better workflows asset revisions 

we need a way to automatically billboard, and to automatically normal map a large gltf. But also we need tools in blender to get a good first pass then manually tweak the process. And we need this asset to be clearly derived.

assets should probably live next to blend file, I cant be navigating the file browser on every export.

need tool which automatically detects and ideally fixes missing reflect derives.


# Thu Nov  5 10:41:52 PM EST 2026
need to refactor the lobby and pylon demos' seperate precompile code into something more sophisticated.

I fixed the camera positioning at startup taking a frame to apply. BUT really we need to not render until startup is done. 

We also should figure out whats with the blank screen we get when window first opens.