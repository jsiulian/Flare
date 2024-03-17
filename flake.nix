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

          libadwaita_1_5 = pkgs.libadwaita.overrideAttrs (finalAttrs: prevAttrs: {
            version = "1.5.beta";
            src = with pkgs; fetchFromGitLab {
              domain = "gitlab.gnome.org";
              owner = "GNOME";
              repo = "libadwaita";
              rev = "66edcd31c1acfb5a569db02870d1f7e471946fcf";
              hash = "sha256-ipyifdRwZCO8H7qQOcEHOXW9PT9w+Ix+k374qHRfSjg=";
            };
          });

          blueprint-compiler-git = pkgs.blueprint-compiler.overrideAttrs (finalAttrs: prevAttrs: {
            version = "0.10.1.git";
            src = with pkgs; fetchFromGitLab {
              domain = "gitlab.gnome.org";
              owner = "jwestman";
              repo = "blueprint-compiler";
              rev = "d47955c5a20b2f7cf85ff25a00b02160883aa0b1";
              hash = "sha256-yNFgJpCGu2CrT6J004N4G+Nlt+3/DbEkSGOWkvSn4J8=";
            };
          });

          name = "flare";
        in
        rec {
          packages.default =
            with pkgs;
            stdenv.mkDerivation rec {
              cargoDeps = rustPlatform.importCargoLock {
                lockFile = ./Cargo.lock;
                outputHashes = {
                  "curve25519-dalek-4.1.1" = "sha256-p9Vx0lAaYILypsI4/RVsHZLOqZKaa4Wvf7DanLA38pc=";
                  "libsignal-protocol-0.1.0" = "sha256-p4YzrtJaQhuMBTtquvS1m9llszfyTeDfl7+IXzRUFSE=";
                  "libsignal-service-0.1.0" = "sha256-p0umCPtBg9s4G6RHcwK/tU+RtQE2fFLRHOYt2GmBCtQ=";
                  "presage-0.6.1" = "sha256-hwvFQBl0/Rb46cpGn7yq3qjI/50TYeJDnyas4oBGrWc=";
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
              buildInputs = with pkgs; [ libadwaita_1_5 pkgs.protobuf pkgs.libsecret pkgs.gst_all_1.gstreamer pkgs.gst_all_1.gst-plugins-base pkgs.gst_all_1.gst-plugins-good pkgs.gst_all_1.gst-plugins-bad pkgs.gtksourceview5 pkgs.gtk4 ];
              nativeBuildInputs = with pkgs; [ pkgs.appstream-glib blueprint-compiler-git pkgs.desktop-file-utils pkgs.meson pkgs.ninja pkgs.pkg-config pkgs.wrapGAppsHook4 pkgs.rustPlatform.cargoSetupHook cargo rustc pkgs.glib ];

              PROTOC = "${pkgs.protobuf}/bin/protoc";

              inherit name;
            };
          packages.flare-screenshot = packages.default.overrideAttrs {
            mesonFlags = [ "-Dprofile=screenshot" ];
          };

          devShells.default =
            let
              run = pkgs.writeShellScriptBin "run" ''
                meson compile -C build && ./build/target/debug/${name}
              '';
              run-gdb = pkgs.writeShellScriptBin "run-gdb" ''
                meson compile -C build && gdb ./build/target/debug/${name}
              '';
              check = pkgs.writeShellScriptBin "check" ''
                cargo check
              '';
              i18n = pkgs.writeShellScriptBin "i18n" ''
                meson compile flare-pot -C build
                meson compile flare-update-po -C build
              '';
              prof = pkgs.writeShellScriptBin "prof" ''
                RUSTFLAGS="-C force-frame-pointers=yes" meson compile -C build
                sysprof-cli --force --no-battery --use-trace-fd --speedtrack --gtk $@ flare.syscap -- ./build/target/debug/${name}
              '';
            in
            with pkgs;
            pkgs.mkShell {
              src = ./.;
              buildInputs = self.packages.${system}.default.buildInputs;
              nativeBuildInputs = with pkgs; self.packages.${system}.default.nativeBuildInputs ++ [ gdb clippy sysprof cargo-deny ] ++ [ run run-gdb check i18n prof ];
              shellHook = ''
                # Required for the application findings settings.
                export GSETTINGS_SCHEMA_DIR=${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas/:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}/glib-2.0/schemas/:./build/data/
                # Required for prost, which by default ships its own protoc (at least in the version of one of our dependencies uses); overrides it to system-protobuf.
                export PROTOC=${pkgs.protobuf}/bin/protoc
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
