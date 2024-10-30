# Compilation

There are different ways to compile Flare.
This document lists some of them.

## GNOME Builder

You should be able to directly clone, compile and run Flare using [GNOME Builder](https://apps.gnome.org/Builder/). Note that if you installed GNOME Builder as a Flatpak, it cannot automatically install the dependencies Flare requires; you need to manually install them in that case using:

```bash
flatpak remote-add --if-not-exists gnome-nightly https://nightly.gnome.org/gnome-nightly.flatpakrepo  # Add the GNOME nightly repository, see <https://wiki.gnome.org/Apps/Nightly>
flatpak install org.gnome.Sdk//master
flatpak install org.freedesktop.Sdk.Extension.rust-stable//23.08
```

## Meson

Compile Flare using the following commands:

```bash
meson setup build -Dprofile=development   # Run once before all changes you make. Substitute "development" for "default" for compiling for release.
meson compile -C build   # Run every time you want to test your changes
GSETTINGS_SCHEMA_DIR=./build/data/ RUST_LOG=flare=trace ./build/target/debug/flare   # Run your locally compiled application with some logging
```

This also requires installation of the dependencies from your package manager. Meson will inform you about any missing packages. Note that Flare ties to keep up-to-date with its dependencies, this might mean that the dependencies provided by fixed-release distros may be outdated. In that case, use one of the other ways of compilation mentioned below.

## Flatpak via fenv

As an alternative, [fenv](https://gitlab.gnome.org/ZanderBrown/fenv) allows you to set up a flatpak
environment from the command line and execute commands in that environment.

First, install fenv:

```sh
# Clone the project somewhere on your system
git clone https://gitlab.gnome.org/ZanderBrown/fenv.git

# Move into the folder
cd fenv

# Install fenv with Cargo
cargo install --path .
```

You can now discard the `fenv` directory if you want.

After that, move into the directory where you cloned Flare and set up the project:

```sh
# Set up the flatpak environment
fenv gen build-aux/de.schmidhuberj.Flare.Devel.json

# Launch a shell inside the build environment
fenv shell
```

You can now follow the compilation phase for GNU/Linux

## Nix Flake

The `flake.nix` defines a devshell for development with this project.
Just activate the devshell and type `run` to compile and execute the program.
