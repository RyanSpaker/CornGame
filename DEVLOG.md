# Fri Mar 21 01:35:26 AM EDT 2025
[first]: #fri-mar-21-013526-am-edt-2025

Ryan, I am creating a dev log file. 
- If you add to it, include a timestamp header before additions. (I use unix `date` command)
- If you want to extend a note later, refer to the previous with a link
  - like [this](#fri-mar-21-013526-am-edt-2025) (vscode can tab complete)
  - or if you want to name a note, like [This][First]

I will mostly use it for links to usefull things. And bugs/limitations/todos.

# Fri Mar 21 01:59:36 AM EDT 2025
issue: dpi bevy
issue: bevy_editor_pls panic on change window scale

- [bevy_editor_pls] ui scale is too small on my screen. 
  - bevy_editor_pls uses bevy_egui and the default [EguiContext] attached to the bevy [Window]
  - Window is a Component and has a WindowResolution which has scale_factor (1.0) and base_scale_factor (1.0) and scale_factor_override (None)
  - bevy uses winit [https://docs.rs/winit/latest/winit/window/struct.Window.html#method.scale_factor]
    - the WINIT_X11_SCALE_FACTOR=2.0 works, as does the randr method, I must have a bad Xft.dpi
    - removed Xft.dpi, but bevy scale still wrong unless run with `WINIT_X11_SCALE_FACTOR=randr` **weird**
    - turns out winit reads from xsettingsd first, reboot and it works.

bevy has a [UiScale] resource, but this doesn't seem to effect egui or bevy_editor_pls 

changing scale_factor in bevy_editor_pls causes *panic* in render code. 
I think because it somehow sets scale to 0, the moment I type "2" because 2 is not an integer.
Same if attempting to set an override (because of Some(0.0))
```
thread '<unnamed>' panicked at /home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/wgpu-23.0.1/src/backend/wgpu_core.rs:2976:18:
wgpu error: Validation Error

Caused by:
  In RenderPass::end
    In a set_viewport command
      Viewport has invalid rect Rect { x: 336.0, y: 92.0, w: 1336.0, h: 2060.0 }; origin and/or size is less than or equal to 0, and/or is not contained in the render target (1820, 2060, 1)
```

alternatively when dragging instead of typing
```
thread 'Compute Task Pool (11)' panicked at /home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/bevy_render-0.15.3/src/camera/projection.rs:229:14:
Failed to update PerspectiveProjection: width and height must be positive, non-zero values: Infinite
```

I am able to change it by pressing up and down keys, further suggesting the issue is with bevy editor pls sending invalid values.

*I have set scale factor to 1.5 manually in [./src/app/ui/console.rs]*

TODO:
- [ ] scale ui with keybinds
- [ ] persist scale setting (and general editor state) on restart

[bevy_editor_pls]: https://docs.rs/bevy_editor_pls/latest/bevy_editor_pls/
[EguiContext]: https://docs.rs/bevy_egui/latest/bevy_egui/struct.EguiContext.html 
[Window]: https://docs.rs/bevy/latest/bevy/prelude/struct.Window.html
[UiScale]: https://docs.rs/bevy/latest/bevy/prelude/struct.UiScale.html

# Fri Mar 21 04:17:27 AM EDT 2025
networking development

I have switched the code to [lightyear]

Currently I have server entities replicating to client but their components are staying synced.
Using [testcube].

I think the problem is I am inserting ServerReplicate on the client. I need to check whether I am the client of the server.
Oh, and I need to be carefull this runs *after* the network is initialized... or else is_server will be false.
- Nope, still an issue, [NetworkIdentity]::is_server() is always false, even though the server is definitely running.
- Setting <Client|Server>Config.shared.mode=HostServer seems to be required
- including setting ClientServer.share.mode=HostServer on the *server* even with no client running.

works at a basic level (Transform synced, cubes moving).

what is the deal with interpolation/predition?

[testcube]: ./src/app/loading/mod.rs#TestCube
[lightyear]: https://github.com/cBournhonesque/lightyear
[NetworkIdentity]: https://docs.rs/lightyear/latest/lightyear/shared/plugin/struct.NetworkIdentity.html

# Fri Mar 21 11:35:32 AM EDT 2025
issue: bevy_editor_pls panic clicking on entity which has fallen off the map.

```
thread 'main' panicked at /home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/bevy-inspector-egui-0.28.1/src/restricted_world_view.rs:223:9:
assertion failed: self.allows_access_to_component(component)
```

# Sat Mar 22 06:12:01 PM EDT 2025
blenvy export is fucked. I'm trying to figure out how to use this cargo container asset I found but it uses all these blender features :/ 
guess I need to just make my own. 

https://github.com/kaosat-dev/Blenvy/issues/267

and another issue exporting spot-light data
https://github.com/kaosat-dev/Blenvy/issues/268

I might need to fork blenvy

# Sat Mar 22 06:58:08 PM EDT 2025
I want to learn how to use bevy_reflect so I can add entity commands to the console.

Got it working `add 91v6 bevy_pbr::bevy_pbr::volumetric_fog::VolumetricLight` for example.

Might work on a more fleshed out shell later (tab completion).
but first I need to look into [bevy_remote] 
via the [vscode extension](https://marketplace.visualstudio.com/items?itemName=splo.vscode-bevy-inspector)
- vs code extension is pretty limited, can only view entities and components, not modify or add them.

There is also [bevy_remote_inspector] which appears to be an unrelated to bevy_remote.
- *had to turn off adblocker for this to work*
- has a bug changing Name component interestingly
- has a bug when clicking on certain entities where it goes to a blank page

[bevy_remote]: https://docs.rs/bevy/latest/bevy/remote/index.html
[bevy_remote_inspector]: https://github.com/notmd/bevy_remote_inspector

# Sun Mar 23 04:52:01 AM EDT 2025
volumetric fog

despite what the docs [imply](https://bevyengine.org/news/bevy-0-15/#volumetric-fog-support-for-point-lights-and-spotlights), it seems you must add FogVolume to the scene to get volumetric fog.

# Sun Mar 23 04:52:10 AM EDT 2025

Performance: 
- [ ] Is it possible to have vertex / fragment shaders abort if frame is taking too long to draw?
- [ ] Is it possible to generate billboard dynamically, could we have a heuristic for their error?
  - how much cheaper would a billboard be than the low poly corn? (if at all)
  - [ ] test low poly vs all billboard
- [ ] Is it possible to use meshlets with the corn renderer (using it to dynamically generate LOD models)
  - first test is just to see how many corn stalks we can draw with meshlets alone
  - what is the cost of having more granularity to LOD?
- [ ] subframerate rendering to a skybox texture for faraway objects.
- [ ] what if we had a seperate model for just the top of the corn?

# Sun Apr 13 05:06:30 AM EDT 2025
[art] 
I have drastically reduced the polygons of the mid/low corn LODS. And got the new one loading in the asset loader. Not sure I like it though. Looks less organic even if it's technically closer to the high poly.

I started texturing the corn, but UVs and LOD selection appear broken in our code.
effect looked kinda cool, I have a screenshot.

---

I managed to get a nice scene with the menu music playing at spawn, fading out as you walk into the maze and then medium_size_maze starts playing once you get to the top of the tower, when you first get a up high view of the maze, pylons leading who knows where in the distance, and shipping container.

I also have the ambient wind attenuate while soundtracks are triggered.

It feels fun to explore the maze.

TODO: late game secret way to climb up on the tower roof and walk across the power lines to the pylons (shortcut?, hack?)

---

New idea: rocket ship. Hint at it early with toy rocket ship + very visible moon (use moon position like a clock with something happening at dead overhead)
end game will take off in ship, seeing truely infinite field of corn.
- unlocks lunar map
  - tent replaced with habitat
  - new monster: moon bears
- unlocks new menu scene (on moon, with cheese like wallice and grommit)

# Mon Apr 14 02:24:59 AM EDT 2025
[gameplay]

- [ ] non-euclidian maze.
  - since you can only see the distance at specific locations (ex tower), we can show and hide objects and have the corn maze change as you walk through.
  - networking is similarly easy.
  - objects have (possibly multiple) 4d_alignment, and the player has a 4d coordinate. View is based on range of 4d values. 
    - don't need fade-in, just make sure nothing is visible when it pops in out
  - corn map has gradient in 4d, which updates player loc.
  - since cutting through corn is generally flat in 4d, this implements shortcuts.
  - this would let us hide alot of content, bridging small medium and large mazes into single maze for the story/advanced mode.
    - maze has "random" variability which can be controlled to keep the player away from secrets in early game, lead them astray in mid game, and guide them to secrets if they get struck for too long in the late game.
- [ ] forced perspective on tower by making character get smaller as you ascend tower
  - corn looks too big.
  - might need to change tower size also.
  
[art]
- use music cues for maze levels
  - ex: harpsicord on castle level
  - presence of harpsichord in non-castle level indicates you are near a 4d crossover

- working on another track: Escape on the Fifth Axis
    - for use during intense final sequence to escape maze for real\
    - big missile/rocket silo underground, sparse splotes of corn growing around walkways.
        - possibly make this not even the real/final rocket, this one deposits you outside maze, next to a little rocket/shuttle and that way we can have the wave goodbye for credits
    - at end of sequence (timed with music) we have the "fifth axis" effect, which makes the moon get really big, and the corn field + everything else shrink really small, with sound effect kinda like outer wilds black hole.

# Mon Apr 14 04:41:19 PM EDT 2025
- [ ] spawning inspector panels
- [ ] docking panels
- [ ] switching cameras
  - [ ] follow objects
- [ ] click on objects
- [ ] camera settings 
  - [ ] cli args
- [ ] blender hot reload
  - [ ] spawn?

- [ ] char controller animations
- [ ] interaction
  - [x] animation
  - [x] event
  - [ ] trigger lights / general game state hook
    - [ ] network

# Thu Apr 17 03:35:29 AM EDT 2025
I am implementing picking/interaction and I cannot for the life of me put up with any more ui bullshit. There is a magic extra /2 in the positioning code. I am not looking into why any more. It is either a bevy bug, a documentation inaccuracy, or something unbelievably stupid on my part, bevy's or both.

As far as gameplay, I am implementing a little mechanic where you have to type out actions. For example, the breaker box has a tooltip "f---" and you have to type "flip". I think it will be a nice touch. 
- and be styled to help with ambiance
- gives actions a little more *body*, making it more immersive
  - try frantically typing "unlock" and "open" while the monsters are catching up. 
- opens the door to a minor puzzle mechanic. ie "--t---e?"
  - and a secret / easter egg mechanic, instead of "flip"ing the switch, try "feel" to see what you "find"

# Wed Apr 23 12:53:51 PM EDT 2025
enabled tracy support

https://github.com/wolfpld/tracy
https://github.com/bevyengine/bevy/blob/main/docs/profiling.md#tracy-profiler

had to use a nix-shell to get it to run on nix without `Failed to initialize OpenGL loader!`

seems usefull for profiling. Not so much for logs. Still need a log viewer.

# Sun Apr 27 11:58:40 PM EDT 2025
https://www.youtube.com/watch?v=y84bG19sg6U

# Wed Apr 30 11:22:05 AM EDT 2025
how to store assets. 

Options: 
1. git lfs with ipfs
    https://github.com/sameer/git-lfs-ipfs
    seems janky. last updated 2 years ago
2. git lfs with s3 (hosted on cloudflare)
    we are absolutely not using AWS. Cloudflare has a 10gig free tier for r2.
    https://dbushell.com/2024/07/15/replace-github-lfs-with-cloudflare-r2-proxy/
3. git lfs hosted on my desktop
    1. git submodule for assets, regular git lfs and run a git server on my desktop
    2. git lfs s3 backend, with minio (self-hosted s3)
4. perforce helix
    industry standard, free for small teams
5. **azure devops**
    free for under 5 users, unlimited storage? (250gb in practice)
    https://www.anchorpoint.app/blog/version-control-using-git-and-azure-devops-for-game-projects
5. https://dvc.org/
    new, designed to deal with ML data, works on top of git

we will use azure devops until if/when it presents a problem. Path of least resistence.
I'd simply have to play around with any selfhosted and or non-standard git lfs solution before I trust it, and understand it well enough to teach you how to use it and avoid fucking the repo.

To minimize annoyances, I will make assets and art seperate repos, and submodules.
The code will remain on github for now, along with my forks.

---

azure is bad
lfs over ssh is not supported. so we have to use https

add git-credential-manager to your nix config
add this to .git/config

```
[credential]
	helper = /run/current-system/sw/bin/git-credential-manager
	credentialStore = plaintext
	useHttpPath = true
```

the first time you clone you should be able to use a password
see clone->generate git credentials

# Wed Apr 30 04:20:40 PM EDT 2025
bevy trace breaks asset loader.
this is known
https://discord.com/channels/691052431525675048/742569353878437978/1313534904474140764

```
2025-04-30T20:15:46.837688Z ERROR bevy_asset::processor: Failed to process asset blueprints/breaker.meta.ron: no `AssetLoader` found with the name 'bevy_asset::server::loaders::InstrumentedAssetLoader<bevy_common_assets::ron::RonAssetLoader<blenvy::blueprints::assets::BlueprintPreloadAssets>>'
```
