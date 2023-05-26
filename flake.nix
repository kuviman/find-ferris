# https://scvalex.net/posts/63/
{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    rust-overlay.url = "github:oxalica/rust-overlay";
    # This must be the stable nixpkgs if you're running the app on a
    # stable NixOS install.  Mixing EGL library versions doesn't work.
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk, rust-overlay, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust-toolchain = pkgs.rust-bin.stable."1.69.0".minimal;
        naersk-lib = pkgs.callPackage naersk {
          rustc = rust-toolchain;
          cargo = rust-toolchain;
        };
        name = "find-ferris";
        waylandDeps = with pkgs; [
          libxkbcommon
          wayland
        ];
        xorgDeps = with pkgs; [
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
        ];
        libDeps = with pkgs; waylandDeps ++ xorgDeps ++ [
          alsa-lib
          udev
          libGL
          xorg.libxcb
        ];
        nativeBuildDeps = with pkgs; [ pkg-config ];
        buildDeps = with pkgs; libDeps ++ [ xorg.libxcb ];
        libPath = pkgs.lib.makeLibraryPath libDeps;
      in
      {
        # defaultPackage = naersk-lib.buildPackage {
        #   singleStep = true;
        #   src = ./.;
        #   doCheck = true;
        #   pname = name;
        #   nativeBuildInputs = nativeBuildDeps ++ [ pkgs.makeWrapper ];
        #   buildInputs = buildDeps;
        #   postInstall = ''
        #     wrapProgram "$out/bin/${name}" --prefix LD_LIBRARY_PATH : "${libPath}"
        #   '';
        # };
        defaultPackage = pkgs.rustPlatform.buildRustPackage {
          name = name;
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = let geng-version = "0.14.0"; in {
              "batbox-${geng-version}" = "sha256-9cbbiLRIjBxb9vc1ldYAXJnr6Jwo+vqyQjNNCDGMZeM=";
              "gilrs-0.10.2" = "sha256-qy0heow6rd8D4anVWfZCHetE4xhLPLXrJSPKJKESh1g=";
              "rodio-0.17.0" = "sha256-nF2cOoPnlfvPkA3JdQPab29YzyKfUEnuF00yJGnVbV8=";
            };
          };
          nativeBuildInputs = nativeBuildDeps ++ [ pkgs.makeWrapper ];
          buildInputs = buildDeps ++ [ rust-toolchain ];
          preBuild = ''
            cargo build --release
          '';
          postInstall = ''
            cp -r ${./assets} $out/bin/assets
            wrapProgram "$out/bin/${name}" \
              --set WINIT_UNIX_BACKEND x11 \
              --prefix LD_LIBRARY_PATH : "${libPath}"
          '';
        };

        defaultApp = utils.lib.mkApp {
          drv = self.defaultPackage."${system}";
        };

        devShell = with pkgs; mkShell {
          nativeBuildInputs = nativeBuildDeps;
          buildInputs = buildDeps ++ [
            cargo
            rustPackages.clippy
            rustfmt
            rust-analyzer
          ];
          shellHook = ''
            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${libPath}"
            export WINIT_UNIX_BACKEND=x11 # TODO fix
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
