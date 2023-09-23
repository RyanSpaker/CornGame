#define_import_path corn_game::lod

#if OVERRIDE_LOD_COUNT == 1
    const LOD_COUNT = 2u;
    const INDIRECT_COUNT = 5u;
#else if OVERRIDE_LOD_COUNT == 2
    const LOD_COUNT = 3u;
    const INDIRECT_COUNT = 10u;
#else if OVERRIDE_LOD_COUNT == 3
    const LOD_COUNT = 4u;
    const INDIRECT_COUNT = 15u;
#else if OVERRIDE_LOD_COUNT == 4
    const LOD_COUNT = 5u;
    const INDIRECT_COUNT = 20u;
#else if OVERRIDE_LOD_COUNT == 5
    const LOD_COUNT = 6u;
    const INDIRECT_COUNT = 25u;
#else if OVERRIDE_LOD_COUNT == 6
    const LOD_COUNT = 7u;
    const INDIRECT_COUNT = 30u;
#else if OVERRIDE_LOD_COUNT == 7
    const LOD_COUNT = 8u;
    const INDIRECT_COUNT = 35u;
#else if OVERRIDE_LOD_COUNT == 8
    const LOD_COUNT = 9u;
    const INDIRECT_COUNT = 40u;
#else if OVERRIDE_LOD_COUNT == 9
    const LOD_COUNT = 10u;
    const INDIRECT_COUNT = 45u;
#else if OVERRIDE_LOD_COUNT == 10
    const LOD_COUNT = 11u;
    const INDIRECT_COUNT = 50u;
#else
    const LOD_COUNT = 2u;
    const INDIRECT_COUNT = 5u;
#endif