# FIXME XXX, THIS FILE IS SYMLINKED AND HARD LINKED!!!

{
  description = "Rust environment example";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: with inputs;
    flake-utils.lib.eachDefaultSystem (system:
      let
        rust = fenix.packages.${system};
        pkgs = import nixpkgs { 
          inherit system;
          config.allowUnfree = true; 
        };
          buildInputs = with pkgs; [
            pkg-config
            openssl
            openssl.dev
            udev.dev
            cacert
            sqlite # for matrix_rust_sdk
            sqlite.dev
            rustc.llvmPackages.clang
						stdenv.cc.cc


            alsa-lib alsa-lib.dev
            vulkan-tools vulkan-headers vulkan-loader vulkan-validation-layers
            lutris
            xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr # To use the x11 feature
            libxkbcommon
          ];
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        devShells.default = pkgs.mkShell{
          name = "rust environment";


					inherit buildInputs;
          nativeBuildInputs = with pkgs; [
            nixd
            rust-analyzer
            rust.complete.toolchain
            pkg-config

            cargo-limit # cargo lcheck, prioritize errors
            cargo-machete #check for unused deps in cargo.toml
            cargo-llvm-lines #info on generic function copies
            cargo-bloat #what takes up space in exe, plus --time flag 
            cargo-features-manager #disable unused features
            cargo-hakari # workspace management
            #cargo-add-dynamic # convert deps to dyn

            sccache
            mold-wrapped
          ];

          # needed for rust-analyzer
          RUST_SRC_PATH = "${rust.complete.rust-src}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          #RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
					LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";

          # https://github.com/rust-lang/rustc_codegen_cranelift
          #CARGO_PROFILE_DEV_CODEGEN_BACKEND = "cranelift";
          RUSTFLAGS = "-C link-arg=-fuse-ld=mold -C linker=clang";
        };
      }
    );
}
