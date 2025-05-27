{ 
  pkgs ? import <nixpkgs> {},
  fenix ? import (fetchTarball "https://github.com/nix-community/fenix/archive/main.tar.gz") {}
}:
with fenix;
with pkgs;

mkShell rec {
  buildInputs = [
    cargo-udeps #check for unused deps in cargo.toml
    cargo-workspaces #list workspace members
    (combine [ # Get nightly rust components as well as the wasm32 target
      (complete.withComponents [
        "cargo"
        "clippy"
        "rust-src"
        "rustc"
        "rustfmt"
      ])
      targets.wasm32-unknown-unknown.latest.rust-std
    ])
    (rustPlatform.buildRustPackage rec { # runner for wasm32 target. automatically creates server to run game
      pname = "wasm-server-runner";
      version = "0.6.3";

      src = fetchCrate {
        inherit pname version;
        hash = "sha256-4NuvNvUHZ7n0QP42J9tuf1wqBe9f/R6iJAGeuno9qtg=";
      };

      cargoHash = "sha256-aq4hrZPRgKdRNvMrE9Lhy3AD7lXb/UocNUNpeNZz3cM=";
      cargoDepsName = pname;
    })
    rust-analyzer
    rustc.llvmPackages.clang 
    rustc.llvmPackages.bintools
    pkg-config

		udev udev.dev alsa-lib
    vulkan-tools vulkan-headers vulkan-loader vulkan-validation-layers
		lutris
    xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr # To use the x11 feature
		libxkbcommon
		
		git
    openssh
    openssl.dev
    cacert
    which
    (wrapBintoolsWith { bintools = mold; })
  ];
  LIBCLANG_PATH = lib.makeLibraryPath [ rustc.llvmPackages.libclang.lib ];
  #PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.udev.dev}/lib/pkgconfig:${pkgs.udev.dev}/share/pkgconfig";
  RUSTFLAGS = "-C link-arg=-fuse-ld=mold -C linker=clang -Zshare-generics=y";
  RUST_SRC_PATH = "${complete.rust-src}/lib/rustlib/src/rust/library";
  LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
}
