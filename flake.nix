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
                  "curve25519-dalek-4.0.0" = "sha256-KUXvYXeVvJEQ/+dydKzXWCZmA2bFa2IosDzaBL6/Si0=";
                  "libsignal-protocol-0.1.0" = "sha256-FCrJO7porlY5FrwZ2c67UPd4tgN7cH2/3DTwfPjihwM=";
                  "libsignal-service-0.1.0" = "sha256-IKRI5Q+goRKyiEc68vWRnF10uToZz2nS9xHCyV/WcKI=";
                  "presage-0.6.0-dev" = "sha256-/WcGqFXijR/C0/Rh91muo3cG8uwPYg9PGkNehrOZEug=";
                };
              };
              src = let fs = lib.fileset; in fs.toSource {
                root = ./.;
                fileset =
                  fs.difference
                    ./.
                    (fs.unions [
                      (fs.maybeMissing ./result)
                      (fs.maybeMissing ./build)
                      ./flake.nix
                      ./flake.lock
                    ]);
              };
              buildInputs = with pkgs; [ pkgs.libadwaita pkgs.protobuf pkgs.libsecret pkgs.gst_all_1.gstreamer pkgs.gst_all_1.gst-plugins-base pkgs.gst_all_1.gst-plugins-good pkgs.gst_all_1.gst-plugins-bad pkgs.gtksourceview5 pkgs.gtk4 ];
              nativeBuildInputs = with pkgs; [ pkgs.appstream-glib pkgs.blueprint-compiler pkgs.desktop-file-utils pkgs.meson pkgs.ninja pkgs.pkg-config pkgs.wrapGAppsHook4 pkgs.rustPlatform.cargoSetupHook cargo rustc pkgs.glib ];

              inherit name;
            };
          packages.flare-screenshot = packages.default.overrideAttrs {
            mesonFlags = [ "-Dprofile=screenshot" ];
          };

          devShells.default =
            let
              run = pkgs.writeShellScriptBin "run" ''
                export GSETTINGS_SCHEMA_DIR=${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas/:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}/glib-2.0/schemas/:./build/data/
                meson compile -C build && ./build/target/debug/${name}
              '';
              run-gdb = pkgs.writeShellScriptBin "run-gdb" ''
                export GSETTINGS_SCHEMA_DIR=${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas/:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}/glib-2.0/schemas/:./build/data/
                meson compile -C build && gdb ./build/target/debug/${name}
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
              nativeBuildInputs = with pkgs; self.packages.${system}.default.nativeBuildInputs ++ [ gdb clippy ] ++ [ run check i18n run-gdb];
              shellHook = ''
                meson setup -Dprofile=development build
              '';
            };
          apps.default = {
            type = "app";
            inherit name;
            program = "${self.packages.${system}.default}/bin/${name}";
          };

          packages.makeScreenshot =
            let
              nixos-lib = import (nixpkgs + "/nixos/lib") { };
            in
            nixos-lib.runTest {
              name = "screenshot";
              hostPkgs = pkgs;
              imports = [
                {
                  nodes = {
                    machine = { pkgs, ... }: {
                      boot.loader.systemd-boot.enable = true;
                      boot.loader.efi.canTouchEfiVariables = true;

                      services.xserver.enable = true;
                      services.xserver.displayManager.gdm.enable = true;
                      services.xserver.desktopManager.gnome.enable = true;
                      services.xserver.displayManager.autoLogin.enable = true;
                      services.xserver.displayManager.autoLogin.user = "alice";

                      virtualisation.qemu.options = [ "-device VGA,edid=on,xres=1920,yres=1080" ]; # Source: https://wiki.archlinux.org/title/QEMU

                      users.users.alice = {
                        isNormalUser = true;
                        extraGroups = [ "wheel" ];
                        uid = 1000;
                      };

                      system.stateVersion = "22.05";

                      environment.systemPackages = [
                        self.packages.${system}.flare-screenshot
                      ];

                      systemd.user.services = {
                        "org.gnome.Shell@wayland" = {
                          serviceConfig = {
                            ExecStart = [
                              ""
                              "${pkgs.gnome.gnome-shell}/bin/gnome-shell"
                            ];
                          };
                        };
                      };
                    };
                  };

                  testScript = { nodes, ... }:
                    let
                      lib = pkgs.lib;
                      l = lib.lists;

                      user = nodes.machine.users.users.alice;
                      username = user.name;
                      uid = toString user.uid;

                      bus = "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/${uid}/bus";
                      su = command: "su - ${user.name} -c '${command}'";

                      key = key: "machine.send_key(\"${key}\")";
                      sleep = duration: "machine.sleep(${toString duration})";

                      execution = [
                        (l.replicate 5 (key "tab"))
                        (key "ret")
                      ];

                      preExecution = [
                        (sleep 10)
                        (key "esc")
                        "machine.succeed(\"${launch}\")"
                        (key "esc")
                      ];

                      postExecution = [
                        (key "alt-print") # XXX: This for some reason sometimes fails. No idea why.
                        "machine.execute(\"mv /home/${username}/Pictures/Screenshots/* screenshot.png\")"
                        "machine.copy_from_vm(\"screenshot.png\", \".\")"
                      ];

                      fullExecution = l.flatten [preExecution (sleep 5) execution (sleep 5) postExecution];

                      keysequence = (lib.lists.replicate 5 "tab") ++ [ "ret" ];
                      executekeys = lib.concatStringsSep "\nmachine.sleep(${sleep.short})\n" (map (x: "machine.send_key(\"${x}\")") keysequence);

                      code = lib.concatStringsSep "\nmachine.sleep(1)\n" fullExecution;

                      # Start Flare
                      launch = su "${bus} gapplication launch de.schmidhuberj.Flare";
                    in
                      code;
                }
              ];
            };

          formatter = pkgs.nixpkgs-fmt;
        })
    );
}
