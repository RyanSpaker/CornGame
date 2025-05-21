- Different parts of the app require different but overlapping things to be "loaded"
- A part of the app is a specific state. Switching states requires running/loading different things
- Loading can be a multiframe ordeal
- Adding A loading dependency: register a Loading Dependency struct to a specific enum variant
- Loading state should be a state resource for each enum that can have loading dependencies
- Some central resource that stores loaded dependencies and can determine necessary dependencies
- Some form of heirarchy for child states
- Preferably we dont have to manually add load state enums for each enum with load depedencies
- Way to specify certain states as load blocking, and certain dependencies as non load blocking


- Example: Prop used only in level 1
- Dependency should be specific only to the LevelState::Level1 type-
- Should only require adding a register_loading_dependency call


- Concrete:
- Loading Tasks are entities. upon completion they have a bool turned true


How to know when to spawn loading tasks
How to know how to schedule systems that complete loading tasks
How to know when a loading boundary has been cleared
How to easily block app progress when loading is not done


- Blocking: Since blocking requires loadingstate enums and specialized scheduling, only specialized loading states will block. Loading tasks can be forced to not block though.
- Nonblocking load dependencies simply spawn in 

- Auto spawn a loading state for states that have a load dependency added to them
- 