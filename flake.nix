{
  description = "Chat with your friends on Signal";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils, ... }@inputs:
    (flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };
          name = "flare";
        in
        rec { 
          packages.default = 
            with pkgs;
            stdenv.mkDerivation rec {
              cargoDeps = rustPlatform.importCargoLock {
                lockFile = ./Cargo.lock;
                outputHashes = {
                  "curve25519-dalek-3.2.1" = "sha256-0hFRhn920tLBpo6ZNCl6DYtTMHMXY/EiDvuhOPVjvC0=";
                  "libsignal-protocol-0.1.0" = "sha256-VQwrGTNZnlDK5p8ZleAZYtbzDiVTHxc93/CRlCUjWtE=";
                  "libsignal-service-0.1.0" = "sha256-1ub0IPSvGhZ2tsC6IolusJ1NSWy+5SXSx8qlIdPngTE=";
                  "presage-0.6.0-dev" = "sha256-4isKBn/4yHoAYsYbBTULK/veZmaecU7t+PvE4Y0oNgk=";
                };
              };
              src = ./.;
              buildInputs = with pkgs; [ libadwaita protobuf libsecret gst_all_1.gstreamer gst_all_1.gst-plugins-base gst_all_1.gst-plugins-good gst_all_1.gst-plugins-bad gtksourceview5 libspelling ];
              nativeBuildInputs = with pkgs; [ appstream-glib blueprint-compiler desktop-file-utils meson ninja pkg-config wrapGAppsHook4 rustPlatform.cargoSetupHook cargo rustc  ];

              inherit name;
            };
          devShells.default =
            let 
              run = pkgs.writeShellScriptBin "run" ''
                export GSETTINGS_SCHEMA_DIR=${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas/:./build/data/
                meson compile -C build && ./build/target/debug/${name}
              '';
              check = pkgs.writeShellScriptBin "check" ''
                cargo clippy
              '';
            in
            with pkgs;
            pkgs.mkShell {
              src = ./.;
              buildInputs = self.packages.${system}.default.buildInputs;
              nativeBuildInputs = with pkgs; self.packages.${system}.default.nativeBuildInputs ++ [ run check ];
              shellHook = ''
                meson setup -Dprofile=development build
              '';
            };
          apps.default = {
            type = "app";
            inherit name;
            program = "${self.packages.${system}.default}/bin/${name}";
          };
        })
    );
}
