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
    pkg-config

		udev udev.dev alsa-lib
    vulkan-tools vulkan-headers vulkan-loader vulkan-validation-layers
		lutris
    xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr # To use the x11 feature

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
