This is for gettting the vscode blender extension debug addon working with blender in nixos.
Since the blender part of the extension tries to install pip deps.

This *does* work, but I've noticed the debugger sometimes gets broken. ie. errors cause future breakpoints to not be tripped, even though debugger is still connected. requiring restarting blender.
