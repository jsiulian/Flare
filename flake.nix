{
  description = "Chat with your friends on Signal";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.nixpkgsgnome.url = "github:NixOS/nixpkgs/d3eb30e4b2205e440e8b77ba1af1632d929e2fcc";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, nixpkgsgnome, flake-utils, ... }@inputs:
    (flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };
          pkgsgnome = import nixpkgsgnome {
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
              buildInputs = with pkgs; [ pkgsgnome.libadwaita pkgsgnome.protobuf pkgsgnome.libsecret pkgsgnome.gst_all_1.gstreamer pkgsgnome.gst_all_1.gst-plugins-base pkgsgnome.gst_all_1.gst-plugins-good pkgsgnome.gst_all_1.gst-plugins-bad pkgsgnome.gtksourceview5 pkgsgnome.gtk4 ];
              nativeBuildInputs = with pkgs; [ pkgsgnome.appstream-glib pkgsgnome.blueprint-compiler pkgsgnome.desktop-file-utils pkgsgnome.meson pkgsgnome.ninja pkgsgnome.pkg-config pkgsgnome.wrapGAppsHook4 pkgsgnome.rustPlatform.cargoSetupHook cargo rustc pkgsgnome.glib ];

              inherit name;
            };
          devShells.default =
            let 
              run = pkgs.writeShellScriptBin "run" ''
                export GSETTINGS_SCHEMA_DIR=${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas/:${pkgsgnome.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgsgnome.gsettings-desktop-schemas.name}/glib-2.0/schemas/:./build/data/
                meson compile -C build && ./build/target/debug/${name}
              '';
              check = pkgs.writeShellScriptBin "check" ''
                cargo check
              '';
              i18n = pkgs.writeShellScriptBin "i18n" ''
                meson compile flare-pot -C build
                meson compile flare-update-po -C build
              '';
            in
            with pkgs;
            pkgs.mkShell {
              src = ./.;
              buildInputs = self.packages.${system}.default.buildInputs;
              nativeBuildInputs = with pkgs; self.packages.${system}.default.nativeBuildInputs ++ [ run check i18n ];
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
