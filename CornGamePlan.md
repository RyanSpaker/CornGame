# Rendering:
The game is going to be designed to be as efficient at rendering 
as possible, meaning optimizations are abundant.

Something to keep in mind, the only complicated thing being 
rendered atm that needs optimization is the corn.
Everything else in the game will be kept simple to allow the 
corn to take center stage, so most resources can be diverted 
to corn rendering

List of Features:
===

Frustum Culling
---
The most baasic optimization is to render only the corn that can 
be seen. 
Corn behind the camera doesn't even need to be sent to the gpu.

- Corn is frustum culled in a per frame compute shader in tandem 
with lod grouping

- A possible optimization is to model the corn fields as positions 
on a flat map, where we can
render a triangle onto the map that uses the cameras horizontal 
bounds as sides, then we can sample this 
map in the compute shader to figure out if a corn field should be
rendered rather than checking per piece of corn

- if the resolution can be small, we can calculate this map on the 
cpu then send it to the compute shader to avoid pipeline changes

LOD Grouping
---
A common optimization is to render corn that is far away at lower 
levels of detail than corn that is closer.
At the extremes, corn that is super far away can be rendered as a 
bill board rather than a mesh.

- Corn is seperated into seperate lod levels in the per frame 
compute pass
- Each LOD uses a seperate draw command
- A MultiDrawMeshIndirect command is used to render the corn. For 
maximum compatibility the global mesh containing all lods will 
need to have an attribute containing their lod id, probably 
connected to their material id attribute. This is so that the 
vertex shader knows which lod level it is rendering, and thus can 
access its lod level instance offset. right now the instance 
indexes start at their offset, but this only works with vulkan
- LOD's can be optimized to be grouped based on the frustum cull 
map. If the vertex at the camera has a value of 1 and the other two 
a value of 0, the map will interpolate between the two values, 
giving a rough approximation of distance in the map. This makes it 
so that the lod grouping doesn't have to calculate distance squared 
per corn. Since distance squared per corn is not a very slow 
operation, it may not be faster, however if we are already sampling 
for frustum culling, it would be a no-op to include lod grouping 
with this method as well.
- At the lowest level the corn will be rendered as a billboard. The 
Billboard system will be its own complex idea that renderes the corn 
in decreasing levels of detail, to be able to extend corn fields to 
infinite length.

Billboarding
---
Far away parts of the corn field will be rendered using a flat 
2-triangle quad. As the corn gets farther away, the billboards 
become responsible for increasingly larger portions of corn.
### Open Questions:
- How do we render corn fields from a dynamic viewing angle? For 
instance, billboards for looking at corn straight on are trivial, 
we may need a few different variations for the corn at different 
angles which can be dynamically swapped out. This system breaks 
down for looking at the corn from the top, such as looking at it 
from a high place. This needs to be possible for multiple places 
in the final game. A possible method is to render a large number 
of billboards that correspond to different viewing angles and 
rotations.
- Viewing billboards from the top will have some Depth Fighting 
issues. Billboards from the front wont have fighting because corn 
will either be entirely behind or entirely in front of other stalks. 
This doesn't apply to corn from the top, as neighboring billboards 
will have depth values overlapping mostly. We could write the depth 
to the prerendered images, and then offset the depth of the fragments 
by the image depth. This would allow neighboring billboards to depth 
fight acccurately. All of this will have major performance implications 
and the most performant method must be found.

**NOTE:** *This issue only exists if the corn stalks cylindrical 
bounding volumes intersect. This means we can hexagonaly pack the corn 
to achieve maximum density, while also eliminating depth fighting issues.*

- The system will need to do a balancing act between gpu memory 
resources, draw calls, and texture reads. 
- Another Question is how to render the horizon in a potentially 
infinite cornmaze. Obviously we wouldn't render infinite corn, the 
horizon would limit to some pattern. If we can find this pattern, we 
can create infinite corn fields relatively cheaply.
- We will probably need to find some way to interpolate between billboard 
textures for the stalks in between rotation and height samples. This is not 
an obvious function, and will take some work to get looking good. The other 
problem is that this will most likely make the total number of samples per 
billboard go to 4 rather than 1. This means we may need to just sample a huge 
number of discrete scenarios and forego interpolation for performance. 
Depending on memory cost, this could also keep a lot of quality. By 
conservative estimate, we would need at least 10 discrete heights, and 20 
discrete rotations, so 200 billboard images in all.
### Some Performance Impacts:
- **Alpha Clipping:** A corn billboard will have to be partially opaque 
with certain pixels empty. This will mean we will need alpha clipping, 
or alpha blending to render the billboards correctly. This is a 
performance impact, as every pixel rendered will need to test their alpha.
An alternative is to set empty pixel's depth to extreme values, allowing 
other billboards to render on top. This will require the depth test to 
happen after the fragment shader, so that transparent pixels will be depth 
clipped. This a performance impact as overdraw will be much worse since 
all pixels fragment programs will run no matter what. *In general I think 
alpha clipping is the way to go here.*

Materials
---
The materials used to render the corn meshes must be carefully crafted to 
maximize performance.
- The highest detail meshes will be rendered with PBR materials, but this 
cant be true for lower levels of detail, as those materials are expensive. 
The worse materials could use decreasing levels of quality, such as PBR 
followed by Blinn PHong, followed by diffuse only, followed by flat color.

**NOTE** *Each level of detail can be rendered with a different material 
without any performance impact as each mesh needs to use a different draw 
call anyway. Although I suppose there is the performance of binding a 
material to the shader, but this is small, and comparatively nonexistant 
to the cost of rendering the corn.*

- Another issue is the textures used to render the corn, texture sampling 
is slower than other methods, so lower levels of detail should avoid using 
them, instead relying on vertex attributes to recieve any necessary 
rendering information. This makes billboards a problem as texture sampling 
is necessary for their rendering.
- Materials also pose another problem, as the ability to render specific 
corn fields with different materials would be a nice feature. This 
requires either having mutliple draw calls, or attaching an array of 
materials to the shader. This presents another problem since materials 
often use texture maps, which cant be bound in an array. There are two 
solutions to this, create a 3d texture, where one dimension is used to 
seperate the materials, or manually define multiple sets of texture 
bindings in the shader with shaderdefs selectively deciding how many to 
define. Using a 3d texture would require some system that collects all of 
the individual textures and combines them into one, then copies that data 
to the gpu. 3d textures are probably best, but this is an annoying 
feature to support so it will wait a while.
- The biggest current issue is that each corn part needs to render with a 
different material, however this is only because i havent made any 
textures for the corn. Once the textures are made, i will only need 1 
material for the entire corn, so rendering different parts with different 
materials will most likely not be implemented. I do have to keep in mind, 
that lower levels of detail will need information about their rendering 
passed in without textures, most likely with a vertex atribute or with a 
uniform constant buffer.

Shadows
---
The corn will need to be rendered with shadows at pretty much all distances.
This is a huge problem as rendering the corn twice is not an option.
- All corn meshes will be rendered at lower levels of detail in the shadow 
map, and the lod curve will be sharper, so that corn will be rendered with 
the lowest lod sooner.
- A major problem will be depth biasing, because lower levels of detail may 
be shaped differently in such a way that the actual corn stalk thinks its 
in shadow when its not. This means the shadow bias will most likely need to 
be rather large. Also, since the biasing is only bad at the leaves of the corn 
rather than the base, we can make it so that the shadow bias is larger, the 
higher up it is, so that the corn shadows intersect the ground correctly.
- A possible major optimization is a custom shadow mapping algorithm designed 
to render dynamic shadow quality, with only 1 shadow map. The way it works is 
that the projection is non linear, and pushes points away from the player. This 
will make the shadows a bit curved, but will allow for dynamic quality. Crafting 
the function will require some care, as we dont want it to be computationaly 
expensive.
- For the billboards, shadows can be rendered, but present a huge dilemna, as 
we will need to orient the billboards towards the player in the shadow pass, 
which requires some extra expensive math. Billboards will most likely need to 
forego shadows, and instead try to fake them in some way. An alternative to this 
is presented in the next section, which is a possible way to avoid using billboards.
- Another possible way to make this system, as well as rendering in general faster 
is to use some sort of smart occlusion culling algorithm. However this would never 
work looking down on the corn maze.

Corn Stalk Texture Map
---
Instead of rendering all these billboards for the low level of detail corn, we could
generate a image faking what the maze would look like at a specific angle. This would 
probably be a novel rendering technique, but I think I can do it, I'm smart enough.
#### General Steps:
- The general principle revolves around rendering the corn maze as a single flat image 
rather than a bunch of billboards. The idea comes from the fact the fact that if you were 
to look straight down on the corn, it would appear as a flat image. As the corn got further 
away from you, your angle would increase, so it would look more 3d. so what if we 
render a set of flat images, each a bandpass slice of the corn maze, and then render them 
in a sort of warped way
- Think of it this way, if you had infinite flat slices of the corn maze, you could render 
the corn perfectly by stacking them up, and then rendering just those flat images. Now, 
simplify by making the number of images less than infinite, and you might just get an 
approximation of the render. This might just look good enough for really far away corn.
