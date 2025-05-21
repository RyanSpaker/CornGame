What I want is full generality. This is the correct solution. Anything else is degenerate.
All players are clients, all players are servers. There may be another server.
servers talk to each other, client only talks to local server
if root server disconnects, next server in line becomes root server
in case of a central server, central server serves as the perminant root server
servers are in a tree from the root, each talks to its parent only, parent is aware of all descendants.
servers are aware of siblings, to facilitate reparenting
entities are owned by a server, there are three considerations
1) clients drive forward unreplicated state
2) servers drive forward replicated state
3) servers take input from their clients for states they own
4) servers take input from their parent for states they do not own

extra: mesh side channels. It is possible that it would be helpfull in cases to allow siblings to communicate as well. The rules to keep this coherant are unclear.

ex) character controller
1) animation interpolation (assume this is unknown to servers)
2) animation state, position, velocity interpolated by local server
3) character controller￼
- however, we want to have the option of having a root server own everything for anticheat (not important to us, but in theory good)
- this implies a need for message passing, and if the message passed to the local server works, then it can be forwarded up the chain to the owning server automatically.
- and the local server can still process the message, and avoid waiting for the parent server.
- so, server and client should never touch the same state, likewise, npc should be in the server, and so-on.

So we have for npcs:
- main tick (owning server)
- interpolation tick (both)
- replicating handler (slave server)
 
TODO how does client inputs feed into above.

problem if client ever touches physics is that it can't be moved up the chain.

replication: 
- creation and destruction of entities
- add remove components

Example for given object.
- we want to send over the name of the asset, but not the mesh.
- for scenes it is probably fine to send over all the subentities, this opens the door to easy procedural changes, and it isn't that much data.
- unclear how autoadded components work, unclear how to deal with components which should be synced for some entities and not others.
  - https://docs.rs/bevy_replicon/latest/bevy_replicon/core/dont_replicate/trait.CommandDontReplicateExt.html#tymethod.dont_replicate
Issues: probably want to add in support for ggrs type procedural rollback, for things like physics, with automatic recovery, in order to reduce bandwidth.
- this would prevent need for sending all entities physics updates, for example.
- idea: hierarchical, fallible procedural rollback.

reason to think my design is good:
1) data (components) and entities are either local or synced, they can be muddled or cleanly delineated, but it is factually the case that the information falls in 2 and only 2 categories
2) you have alot of choices in design, and generally, using components as the unit of syncing makes sense. 
3) synced data lives on a network of computers, if the server is where synced data lives, then every computer is part of the "server"
4) keeping client server arch for single or multiplayer means one less thing to think about
5) the symmetry makes more complex multiplayer an option
6) and the symmetry equates to less thinking for the dev
7) from the point of view of client code, messages to the server are messages to the syncing layer.
8) the design of the syncing layer is not important to the client code (some server reads the message, and responds)

The client is all code that doesn't touch networking, the server is all code that does.
Only the server can create or modify replicated components.
Only the client can get user input.

conclusion: bevy replicon is a good *first order* aproximation of what we want, we may fork it or build on top of it to get server fallback and the correct^tm design I describe above (could be upstreamed maybe)

much like my novel software, I'm going to say we play fast and loose with this one.

Starting minimal example:
- get 2 instances of game talking to each other locally
- server side spawns 2 cubes, one for each player
- both control with arrow keys.
