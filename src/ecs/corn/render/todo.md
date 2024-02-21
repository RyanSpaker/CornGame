## Corn Rendering TODO:
- While `scan_prepass.rs` does have some timing info programmed in, It needs to be expanded. It alos needs some comprehensive tests to be added to measure timing info and correctness of the function
- Render needs to have timing info added as well as tests added.
- Render is kinda a wonky solution, not `render.rs` itself, but `specialized_material.rs`. Finding a better way to accomplish this sort of draw function override would be great.
- There are like 20 different rendering tricks to look into, like billboarding, as well as improvements to the quality of the rendering.
- A big bug to work out is the issue of shadow popping. It may be performant to just do a second scan_prepass, creating a second buffer containing the corn that will be rendered to the shadowmap, it could even be added to the first scan_prepass.