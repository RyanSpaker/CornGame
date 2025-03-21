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
Using testcube.

[testcube]: ./src/app/loading/mod.rs#TestCube
[lightyear]: https://github.com/cBournhonesque/lightyear

