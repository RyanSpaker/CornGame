# CornGame
Corn Maze game written in rust with the bevy engine

## TODO:
- clean up and document rendering code
- Switch certain renderable corn field functions to take in a struct, rather than parameters in order to make future changes easier
- Switch corn model loading to use the asset preproccessing pipeline
- Add even more control to the rendering of the corn, including the ability to render each distinct mesh of the corn seperately rather than together if the renderable corn field requests it
- Add a second indirect corn buffer, or extend the first, in order to have the ability to choose a base lod level, which would allow rendering the shadows at a lower lod level than the corn itself
- Switch over to a 
- actual game logic
## FUTURE OPTIMIZATION TASKS: 
- :x: Find a solution for different parts of the corn being rendered differently
- :x: allow different corn fields to be rendered with different materials
- :x: allow different lod levels to be rendered with different materials
- :x: Custom shadow mapping algorithm? maybe
- :x: Some sort of extreme distance billboarding technique
- :x: Some sort of extreme shell texturing technique for really far away corn
- :x: Some sort of skybox limit texture to render an  infinite corn field effect


