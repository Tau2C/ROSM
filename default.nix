{ pkgs ? import <nixpkgs> {} }:
with pkgs;
mkShell {
  packages = [
    rustup
    pkg-config
    fontconfig
    libxkbcommon
    wayland
    libx11
    libxcursor
    libxrandr
    libxi
    libGL
  ];

  LD_LIBRARY_PATH = lib.makeLibraryPath [
    fontconfig
    libxkbcommon
    wayland
    libx11
    libxcursor
    libxrandr
    libxi
    libGL
  ];
}
