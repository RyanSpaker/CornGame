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



## POI
- Specialized Material is a really annoying way to accomplish what i need for instanced rendering. I might want to write a program which converts bevy's built in shader files for PBR rendering, and converts them into simple methods rather than full vertex or fragment code, this would allow us to write true shader extensions, where our vertex or fragment shader is run before or after the built in code. This owuld require some fancy logic, and im not sure if it's even useful, but it could be nice.
- The corn rendering pipeline is currently ignoring timestap writes, but these will probably be extremely useful to get working for performance information
- The current asset system is asinine when it comes to GLTF, but the current branch of bevy im using at least makes my corn asset loader possible. In the future it would be smart to try and get bevy to fix some of the missing functionality so that it isnt so much of a pain
- The current game structure is really disorganized, and needs to be cleaned up