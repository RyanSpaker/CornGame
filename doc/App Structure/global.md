This is a write up of how the app is structured, and why.

## Global Configuration
The base of the game, the main.rs file is responsible for adding our corn game module, as well as any development specific functionality like world inspector. This is so that when we release the game, it is really easy to seperate our debug code from our release code.

The corn game module is then split into 3 core folders, App, ECS, and Util.

Util is the simplest, and is simply the place where we put any stand alone functionality that is needed, but we can't find a library for.

App is a module which defines our application structure, including scene switching, app state, loading, menus, and whatever else defines the application structure.

ECS is a module which contains discrete game element or system. (flashlight, corn, player, map, etc).


### App

The app defines the state machine of the app:

AppState:
- Init: Initial state of the app. Used to setup global app settings like window state and other such nonsense.
- Open(AppMenuState, GameplayState): after initializing, the app moves into the loaded state, which contains a substate to describe the gameplay and menu state.
- - AppMenuState: Describes the state of the app menu
- - - 


- Loading: This is the initial state of the app. we load bare essential assets, and prepare the window.
- Main Menu: This is the state we enter upon finishing loading. The player is presented a main menu, with an options sub menu, and the ability to start the game
- Gameplay: This is the state we enter upon starting the game, It is then broken up into many sub states.

Gameplay: (PauseState, LevelState). Gameplay is split into pause state, to represent whether the game has been paused, with a pause menu being shown when it has, and level state

LevelState: (Level, LoadState). LevelState describes the current level, with the first level being Lobby upon starting the game, and the load state, which is whether the level has been loaded or not.
This means the app can be paused while loading a level, which as far as i can tell is not the case with any game ever made for some reason.

This state machine allows the entire app to be aware of exactly what is happening at all times, which means we can create a large number of system sets, and define their run conditions to easily make certain functions only run during certain times. System sets are vitally important, and any time two systems share common run conditions, they should be added to a system set that defines those conditions. This practice will make system scheduling much better as the app becomes more complicated.

A Consequence of the app state setup is that we also get easy callbacks for when loading or pausing is finished or started with OnEnter and OnExit, 