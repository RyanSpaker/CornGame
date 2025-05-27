## Project Structure

- ### main.rs/lib.rs:
The game is defined in the lib file, making it releasable as a crate. Meanwhile the main file allows us to compile an executable. \
This allows us to easily seperate development tools like world-inspector, and a bevy editor, from the actual game code. \
It also makes it simpler to setup a mod development process and api in the future.

- ### utilities:
This is a module which contains completely self inclosed functionality, which is not directly connected to the corn game. \
These could be their own libraries, or bevy PR's, but are manually made for the project.
- ### states:
This module is structured as a mirror of the state types of the app, and sets up state functionality. \
Mostly includes state-definition, state-specific scheduling setup, and OnEnter/OnExit systems.
- ### modules:
This module contains definitions for "things" in the app, which are mostly self-inclosed, and hook into different sub-systems of the app. \
An example might be a flashlight, which hooks into the equipment sub-system. \
In the future, I think mods should be created mirroring the modules in this folder, so modules here can be thought of as example mods.
- ### systems:
This module contains definitions for sub-systems of the app. \
Ex: Options, Equipment, Assets, Networking, Sound. \
Any functionality that needs some global setup or code, but is used all over the app should be defined here.