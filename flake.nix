{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {

      devShells.x86_64-linux.default = pkgs.mkShell rec {


        nativeBuildInputs = with pkgs; [
          glibc
          pkg-config
          gcc

          # iced

          # gtk-layer-shell
          # gtk3
          # libxkbcommon
          # libGL
          # wayland
          # wayland.dev
          # wayland-protocols
          # vulkan-loader
          # libxkbcommon
        ];

        buildInputs = with pkgs; [
          rustc
          cargo

          pkg-config
          wayland
          libxkbcommon
          libGL

          git
          zsh

          flutter
          ninja
        ];

        LD_LIBRARY_PATH = "${nixpkgs.lib.makeLibraryPath buildInputs}";

        shellHook = ''
          export PATH=~/.cargo/bin:$PATH
          export PROJECT_ROOT=$(git rev-parse --show-toplevel)
          export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$PROJECT_ROOT/gui_flutter/client/build/linux/x64/debug/bundle/lib:$PROJECT_ROOT/gui_flutter/client/build/linux/x64/release/bundle/lib:$PROJECT_ROOT/gui_flutter/build/linux/x64/debug/bundle/lib:$PROJECT_ROOT/pridwen/client/build/linux/x64/debug/bundle/lib:$PROJECT_ROOT/pridwen/client/build/linux/x64/release/bundle/lib:$PROJECT_ROOT/pridwen/build/linux/x64/debug/bundle/lib"
          zsh
          exit;
        '';

      };
    };
}
