# CornGame
Corn Maze game written in rust with the bevy engine

## TODO:
- :heavy_check_mark: basic application structure
- :heavy_check_mark: dynamic creation of corn field instance data on the gpu
- :heavy_check_mark: dynamically merge all corn instance data buffers into one master buffer
- :heavy_check_mark: flag stale data as disabled to not render them: can be worked into the init compute shader for performance
- :heavy_check_mark: systems to shrink, and defragment the instance buffer
- :heavy_check_mark: finalize corn data pipeline file, and cleanup/document code
- :heavy_check_mark: Actually render the corn
- :heavy_check_mark: Fix StandardMaterial/Corn Shader to make it actually render using pbr
- :heavy_check_mark: Allow for different corn fields to have different origins
- :x: Frustum culling, and LOD grouping
- Allow for infinitely many corn fields
- Find a solution for different parts of the corn being rendered differently
- allow different corn fields to be rendered with different materials
- allow different lod levels to be rendered with different materials
- :x: cleanup and finalize scan_prepass.rs, render.rs, and corn_field::mod.rs / Stress Test
- :x: Custom shadow mapping algorithm, only if it is better
- :x: Some sort of extreme distance billboarding technique
- :x: actual game logic

