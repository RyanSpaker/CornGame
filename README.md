# CornGame
Corn Maze game written in rust with the bevy engine

## TODO:
- [x] basic application structure
- [x] dynamic creation of corn field instance data on the gpu
- [x] dynamically merge all corn instance data buffers into one master buffer
- [x] flag stale data as disabled to not render them: can be worked into the init compute shader for performance
- [x] systems to shrink, and defragment the instance buffer
- [ ] finalize corn data pipeline file, and cleanup/document code
- [ ] Frustum culling, and LOD grouping
- [ ] Actually render the corn
- [ ] Custom shadow mapping
- [ ] Some sort of extreme distance billboarding technique
- [ ] actual game logic

