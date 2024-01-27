Networking should be implemented such that very little code actually needs to know about it. 
This is relavtively easy in ECS, as long as you don't do anything dumb.

Our game requires minimal physics. Rapier is overkill but IDK what would be simpler.
Our game requires netcode. It doesn't really *need* rollback netcode, but thats what all the nice libraries are written for.

We almost certainly want to use rapier, ggrs (good game rollback system) and matchbox (p2p webRTC) as a stack.

# GGRS

The big problem is that ggrs, being an "i am very smart" project, fully assumes your game is determinisitic. This will obviously be mostly true, but I've been seeing people with the worlds most simple game spending tons of time tracking down minute desync bugs. I don't want to do this. I don't care if the game desyncs a little as long as it gets resunc before its a huge difference. This is not an FPS.

The issue is I see no info on how to resync in ggrs, only how to detect dsync. We may require an additional layer.

Basically for every object in the game there are a few options
1. it is run by every client (fully determinisitic)
2. it is run by one client, and the rest do rollback (kinda like player input)
3. simple position, velocity rollback with one client in control

Although its not really that clear a distinction, especially without understand ggrs better.

Another distinction, is whether there is a master client, which owns all the nondeterministic stuff (rng, npc's, etc), or whether different clients are a source of truth on different entities.
One option is to have the client whose player is closest to an entity own that entity's state. 
Similar idea is that only players in proximity to an npc actually execute the npc's ai. Otherwise only the position is recieved from the other clients (you do need a fallback in this case for entities which nobody is near).

All of this diverges from ggrs to a point that we may have to roll our own thing. However I think it is worth it as we don't need ggrs's deterministic guarentees. It should also be possible to have ggrs running in addition to other modes of communication. Additionally, ggrs should be agnostic to the kind of data it is rolling back. Normally it would be player's controller inputs, but it could just as easily be raw position. I'm just a bit worried about getting this working correctly with feedforward without creating a feedback loop (and breaking determinism to the point ggrs shits the bed)

# Mesh

Another thing I am unclear on is how matchbox p2p works. The handshake server part is simple, but once a room with 4 players is created.. how do they communicate. Does every client broadcast to every client, or is there some more efficient mesh/gossip protocol. 

Direct connections scale quadratically with the number of players. At 4 players it isn't a problem. At 10 it might be.

# entity serialization

Another consideration is how everything gets bootstrapped, its easy in an example where everything is set up in advanced, but what about when entitys and their systems might be dynamic. Our options are basically in-band and out-of-band

- out-of-band: (this is probably the normal way) The server or master client sends out instructions at the start of the match for what kind of match it is, and throughout the match when new entitys are created. The handler for these messages sets up the entities and makes sure ggrs starts of syncronized.

- in-band: A more general and abstract method would be to write a generic entity sync system. Entities spawned a certain component would be serialized and spawned on all clients. 

The difference is whether the messages passed talk about entities generically or some higher-level game state.
