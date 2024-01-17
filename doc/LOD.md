The single most important performance optimization for a game is level of detail. For corngame this applies not only to the corn, but to all assets we might add to the game.

In typical game development, large amounts of asset creator's time is spent creating these lower LOD assets (often the original hi resolution model is not even used in the game). Because we are not fulltime 3d artists, and are in fact much better at writing code than working in blender, it may save time (especially as more assets are needed for the game) to automate some LOD work by building it into the game engine itself. This computational work could be done at runtime, or baked in ahead of time. We should also be able to mix the two on a per asset basis.

The two many types of LOD we will need in corngame are low resolution models and billboards. There is some question of whether the cost of texture sampling might outweight the benefit of the billboarding approach.

This is a list of potential LOD automation features. Easiest to hardest.

# Automatic LOD cuttoff selection.
Each successive LOD model needs a distance cuttoff where the next LOD is used. Typically these are set manually, and either the cuttoff or the model is tweaked to get an acceptable level of visual flicker.

However, this could be automated. Visual flicker can be quantized by comparing the pixel error between the rasterization of the 2 models.

By rendering two consecutive LOD models from many angles, and looking at the worst case pixel error, we can assign an LOD a quality metric. To choose LOD cuttoffs we start with the highest LOD model and (using a good first guess) intelligently try other cuttoffs untill the configured quality metric is reached (within tolerance).

Converting pixel diffs to errors could be a simple sum, or could take into account scattering (a block of pixels is worse than individual scattered ones).

The quality target can be parameterized in settings. We may also want to take into account a performace heuristic, as well as adjusting the quality target with distance (since far away objects are much more likely to be occluded)

It isn't clear if this would work better rendering with textures or without. Most likely, LOD's should be rendered the same here as in the game. We may also want to treat the depth buffer as a channel for error calculation since changes to model intersection could produce flickering as well.

Note that this entire method is also usefull for **evaluating** lod cuttoffs, *and the lod models themselves*, quantitatively, even if we don't use it to dymanically choose cuttoffs.

# Automatic billboard creation
We can automate the creation of billboards using a similar method to above.

In fact we could even automate the number of angles used for the billboards. My best estimate for number of angles needed is 10 around the axis plus every 15 degrees to vertical. Higher angles of incidence don't need as many axial samples -- vertical only requires one. Assume then it averages to 5 * 13 = 65 billboards.

There is 2 kinds of billboarding, from what I can tell.
1. a single billboard is used, based on view angle. Billboards always face the player.
2. 2 or 3 static perpendicular billboards (minecraft grass). (generally do not face the player)

Both approaches could be tried. The later has nice intersection properties but is less efficient.

# Automatic LOD models

This is more complicated. LOD creation has alot of moving parts, including reducing vertices, baking normals and texture, which involves dealing with UV maps, etc. The algorithms for reducing poly's automatically are complicated and finicky also.

I have an idea for a autoLOD alorithm that uses our LOD error function from above. Basically the idea is typical decimate algorithsm (I am guessing here) look at the 3d model, and try to preserve geometry. 

My idea is to look at the 2d rasters, and try to preserve pixels. I belive this will produce a better LOD model, because it will work using the same way our eyes/brains do, when parsing shape.

This approach could be adapted for near and far LOD's by using the desired cuttoff for the camera distance. This is good because perspective on meshes near the camera might change what the optimal model is. This is especially true for decimating the highest LOD (typically even the highest LOD is lower res than the original blender mesh). 

The algorithm:
- take raster of the current LOD at many angles.
- use an algorithm for getting 3d info from many 2d angles, constrain to use a max number of faces. (this is the part we'd have to invent)
- possibly use some kind of iterative error guided process using the LOD error function we defined.

If actually implemented, we should open source it as bevy-autoLOD because it would make Unity look like fools.

# Roadmap
- [ ] Figure out how to create a seperate render context with access to the same meshes/materials/scenes as the rest of the game. 
- [ ] Figure out how to do LOD with normal bevy objects.
- [ ] Implement the LOD error checker as standalone tool (gltf file as arg). Make it save error images (tiled into one image).
- [ ] Implement LOD cuttoff search using error function and quality/performance options.
- [ ] Integrate LOD cuttoff search into scene/asset loading.
- [ ] Evaluate cost of texture sampling for huge numbers of faraway corn (billboard feasibility)
- [ ] Theoretical evaluation of 2 types of billboarding.
- [ ] Implement billboarding (test with hand drawn billboards)
- [ ] Implement billboard generation (reuse code from LOD tester)
- [ ] Figure out algo for constrained 2d -> 3d.
- [ ] Implement poc raster based autoLOD
- [ ] Integrate autoLOD into asset loading