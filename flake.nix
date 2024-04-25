{
  description = "A very basic flake";

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
          pkg-config
          wayland
          libxkbcommon
          libGL
        ];

        LD_LIBRARY_PATH = "${nixpkgs.lib.makeLibraryPath buildInputs}";

        shellHook = ''
          PATH=$PATH:~/.cargo/bin;zsh;exit;
        '';

      };
    };
}
