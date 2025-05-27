# User Stories

## Main Menu
- Loading into the main menu requires certain entities to be created, such as a camera, a window, etc.
- Certain entities need to be created only while in the main menu
- Main menu is created using UINodes
- Sub menus exist, and their created may have fancier logic than just seperate UINode stacks, such as something similar to The Talos Principle
- Certain Assets are needed such as title/background images.
  
## Lobby
- Corn Model neeeds to be loaded into the app
- Basic shapes and materials should be created
- Lobby specific scene entities should be created
- UI Camera needs to exist for level select
- 3d Camera is needed
  
## Level
- Corm Model is needed
- Basic Shapes
- Level specific entities
- UI and 3d camera

## MainMenu -> Lobby
- Camera and window are not removed.
- Main menu UI is removed
- Lobby is loaded
- Pause menu is created

## Lobby -> Level
- Level select UI and Lobby scene removed
- Pause menu cameras window kept
- Level entities added

# Concerns
- Loading, how do we ensure things are loaded when necessary
- Blocking Loads, How do we block the app from certain states, and show a loading screen when necessary
- How to make certain things that are required in multiple places only load when needed
- How to add state bound load objects
- How to ensure no overhead during regular states

## Idea
- Things that are only required to run at the start of a state can be added to OnEnter(state)
- Things that may be required in multiple places are individually handled as LoadEntities
- 
- When we enter a new state we enter the loading state (if required)
- OnEnter(Loading) -> we check the load requirements and load anything that is required by the app
- we can also unload anything that is no longer required


- Window: Needs to be created at the start of the app, doesnt need to be touched after
- Camera2d: Needed whenever 2d things are drawn. Since menus and things exist in almost all states, should probably just be a global entity
- Camera3d: Needed only during lobby and level, at least for now
- Corn Model: Needed during Lobby/Level
- ECS module: May define models needed during certain states, resources, or other things required to be loaded at certain times
- 