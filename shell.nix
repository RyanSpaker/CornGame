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
    (complete.withComponents [
      "cargo"
      "clippy"
      "rust-src"
      "rustc"
      "rustfmt"
    ])
    rust-analyzer
    rustc.llvmPackages.clang 

		udev udev.dev alsa-lib alsa-lib.dev
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

		pkg-config
  ];

  LIBCLANG_PATH = lib.makeLibraryPath [ rustc.llvmPackages.libclang.lib ];
  RUSTFLAGS = "-C link-arg=-fuse-ld=mold -C linker=clang -Zshare-generics=y";
  RUST_SRC_PATH = "${complete.rust-src}/lib/rustlib/src/rust/library";
  LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
}
