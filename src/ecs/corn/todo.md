# Corn Todo:
- Make sure corn field images are pixel perfect sampled in init shader ? maybe, maybe this isnt what we want
- Allow corn fields to have multi step init methods.
- Make the corn fields seperated into buffers of rectangles of width max_lod distance so that we only have to sort at most 9 of these buffers per frame.


# Corn Redesign:

### Considerations: 
#### Initializing Corn
- Specific Corn Positions should not be specified, instead the corn should be automatically placed in a region using some spacing algorithm.
- Data should be generated once for a corn field and then live on the GPU. 
- One buffer per corn field, so that macro occlusion culling can be performed cheaply.
- Corn Position Does not need to be reproducable, probably.
#### Vertex Shader:
- The vertex shader needs to run for every vertex of rendered corn since transformations need to be applied to each vertex.
- To decrease how many times the vertex shader is run, we need to decrease how much corn is rendered, or use lods.
- Decreasing corn instances is only possible using frustum culling, and maybe occlusion culling for large objects
- Lods each have their own draw call, since they are a different mesh, and need to be dynamically calculated. More lods means less vtcs, but a slower scan pass.
- Super low vertex shader calls can be achieved with billboards which can have as low as 3 vtcs.
- Shader can optimized by calculating instane_matrix*mesh_matrix in the scan shader, and sending the final matrix into the vertex shader. That way scale rotation and translation for all of an instances vtcs will be handled with no extra cost. Wind will still add extra work since it depends on the vtx height.
#### Fragment Shader:
- Fragment shader runs once for every rasterized pixel of a corn stalk, so long as it is in front of the current pixel.
- Reducing fragment shader invocations can be done by reducing how much is drawn, and by reducing overdraw.
- Overdraw can be easily reduced by rendering higher lods first, since they are closer to the camera.
- Reducing drawn geometry is harder, but a good starting point optimization is to look at sub pixel triangles.
- Deferred rendering is also vital as it dramatically reduces the cost of overdraw, since lighting happens once per pixel, rather than once per fragment invocation.
- We dont currently use a custom fragment shader, so making an optimized one could easily boost performance.
#### Scan Prepass:
- In order to render the corn, we need to know how many instances to render, and for a contiguos instance id, get the correct data from the corn buffer.
- This can be accomplished with a vote-scan-compact shader set.
- Vote chooses per corn instance, whether to render it.
- Scan-Compact builds a contigous array of the rendered instances.
- We can build a contigous array of indexes into the original array, or copy the data into a contigous array. Copying will speed up the vertex shader at the cost of slowing the scan-compact shader.
- Vote will have to calculate lods, frustum culling, and any other culling/per instance optimizations.
- Possible enhancement if this stage is slow: only run vote every couple frames, possibly with async shader stages. depends on backend support.
- Make sure copied to buffer is persisted between frames so that we dont reinitialize it every frame
#### Buffer Management:
- Ideally corn fields will be simple entities on the main world with some configuring data.
- A seperate entity will exist in the render world, holding the render data, and data buffer.
- Changing the main world corn field configuration requires remaking the renderworld data.
- The main worlds corn field matrix will be sent to the render world every frame, and will not require a rebuild.