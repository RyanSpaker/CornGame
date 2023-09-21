# CornGame
Corn Maze game written in rust with the bevy engine

## TODO:
- :heavy_check_mark: basic application structure
- :heavy_check_mark: dynamic creation of corn field instance data on the gpu
- :heavy_check_mark: dynamically merge all corn instance data buffers into one master buffer
- :heavy_check_mark: flag stale data as disabled to not render them: can be worked into the init compute shader for performance
- :heavy_check_mark: systems to shrink, and defragment the instance buffer
- :heavy_check_mark: finalize corn data pipeline file, and cleanup/document code
- :x: Frustum culling, and LOD grouping
- :x: Actually render the corn
- :x: Custom shadow mapping
- :x: Some sort of extreme distance billboarding technique
- :x: actual game logic

