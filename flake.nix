{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      system = "x86_64-linux";

      shellInputs = with pkgs; [
        (rust-bin.stable.latest.default.override { extensions = [ "rust-src" ]; })
        clang
        rust-analyzer
      ];
      appNativeBuildInputs = with pkgs; [
        pkg-config
      ];
      appBuildInputs = appRuntimeInputs ++ (with pkgs; [
        udev
        alsa-lib
        vulkan-loader
        xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr # To use the x11 feature
        libxkbcommon wayland # To use the wayland feature
      ]);
      # TODO figure out appRuntimeInputs
      appRuntimeInputs = with pkgs; [
        vulkan-loader
        xorg.libXcursor
        xorg.libXi
        xorg.libXrandr
        wayland
      ];
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = appNativeBuildInputs;
        buildInputs = shellInputs ++ appBuildInputs;

        shellHook = ''
          export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath appRuntimeInputs}"
        '';
      };
      formatter.x86_64-linux = nixpkgs.legacyPackages.x86_64-linux.nixpkgs-fmt;
    };
}
