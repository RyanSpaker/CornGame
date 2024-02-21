# Corn Todo:
- Make sure corn field images are pixel perfect sampled in init shader ? maybe, maybe this isnt what we want
- Allow corn fields to have multi step init methods.
- Make the corn fields seperated into buffers of rectangles of width max_lod distance so that we only have to sort at most 9 of these buffers per frame.