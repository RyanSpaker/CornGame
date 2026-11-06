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
        rustpkgs = with fenix.packages.${system}; combine [
          complete.rust-src
          complete.toolchain
          targets.wasm32-unknown-unknown.latest.rust-std
        ];
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

          alsa-lib alsa-lib.dev
          # CODEX (GPT-6): native network_audio encodes/decodes the browser-compatible Opus wire format.
          libopus libopus.dev
          vulkan-tools vulkan-headers vulkan-loader vulkan-validation-layers
          lutris
          libX11 libXcursor libXi libXrandr # To use the x11 feature
          wayland
          libxkbcommon
        ];
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        inherit rustpkgs;
        devShells.default = pkgs.mkShell{
          name = "rust environment";

					inherit buildInputs;
          nativeBuildInputs = with pkgs; [
            # CODEX (GPT-6): the lobby's portable web packager uses only Python's standard library.
            python3
            # CLAUDE (Opus 5.5): precompresses the lobby's wasm for nginx (Content-Encoding: br).
            brotli
            nixd
            rustpkgs
            pkg-config
            linuxPackages_latest.perf

            cargo-udeps
            cargo-limit # cargo lcheck, prioritize errors
            cargo-machete #check for unused deps in cargo.toml
            cargo-llvm-lines #info on generic function copies
            cargo-bloat #what takes up space in exe, plus --time flag 
            cargo-features-manager #disable unused features
            cargo-hakari # workspace management
            #cargo-add-dynamic # convert deps to dyn

            # for deps that need to compile c via clang
            llvmPackages.clang
            llvmPackages.bintools

            sccache
            mold-wrapped
            wild

            renderdoc
						
						tracy #-glfw
						libGL

						rgp #radeon graphics profiler
						dbus.lib
          ];

          # needed for rust-analyzer
          RUST_SRC_PATH = "${rustpkgs}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          #RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
					#LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";
					LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
      			pkgs.libxkbcommon
						pkgs.vulkan-loader
            pkgs.renderdoc
						pkgs.dbus.lib
						pkgs.libGL
    			];

          # https://github.com/rust-lang/rustc_codegen_cranelift
          #CARGO_PROFILE_DEV_CODEGEN_BACKEND = "cranelift";
          
          # DO NOT SET RUSTFLAGS HERE: cargo's is retarded and will ignore .cargo/config.toml rustflags instead of appending them
          # https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags
          # RUSTFLAGS = "--cfg tracing_unstable"; # moved linker args to .cargo/config.toml
        };
      }
    );
}
