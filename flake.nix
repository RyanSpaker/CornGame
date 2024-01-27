{
  description = "novel markdown workflow";

  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.fenix.url = "github:nix-community/fenix";

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let 
          pkgs = import nixpkgs { inherit system; config.allowUnfree = true; };
          f = fenix.packages.${system}; 
        in
        {
          devShells.default = import ./shell.nix { inherit pkgs; fenix=f;};
        }
      );
}

