# Contributing

Contributions to Flare are always welcome. There are many different ways you can contribute, this document will show you most of these ways and what you should keep in mind.

## General things

The [GNOME Code of Conduct](https://wiki.gnome.org/Foundation/CodeOfConduct) applies to this project, therefore, be nice to each other.

## Contacting

If you have any issues contributing or just want to chat a bit, feel free to join the [Matrix Channel](https://matrix.to/#/#flare-signal:matrix.org). This is also a good place to ask some questions or report smaller issues.

## Internationalization

This is probably the easiest way to contribute to Flare. Just head over to [Weblate](https://hosted.weblate.org/engage/flare/) and start translating. If you are unfamiliar with Weblate, make sure to read their [documentation](https://docs.weblate.org/en/latest/user/translating.html) on how to translate a project.

There are only a few things to keep in mind with internationalization: 

- Don't just Google-Translate. 
- I will do an announcement about one week before every major release in addition to a string-freeze. This will most likely send you an E-Mail. Please translate the new strings in this time-frame, but if you miss it, this is not a big deal, there will be plenty of time for the next release.

## Open Issues

Issues are a good way to tell me problems you are having with the applications or things that you feel are missing or might be improved. There are a few things to keep in mind with issues.

- Use the correct template: This project defines a few different templates (e.g. "Feature Request", "Error") that you can use for different types of issues. Please only use the default template if you think no other template matches.
- Fill out all information in the template: For me to re-create your setup, it is important that you fill out all information that is asked for in the issue (e.g. your current version, or how you installed it).
- Fill out the logs if you think they are relevant: If the issue template asks for logs and you think that might help, please consider adding them. But note that logs can contain sensitive information. Please look through the logs and censor things before pasting into the template (running the script [here](https://gitlab.com/schmiddi-on-mobile/flare/-/snippets/2538264) should censor your logs for you). If you don't feel comfortable censoring your own logs, you can also send me a Matrix direct message to `@schmiddi:matrix.org` if I ask for logs.
- Check for duplicated issues: Try to use the search-feature if you can find similar issues like you are having. If there is already such an issue, consider giving it a thumbs-up or commenting more details on that issue, but don't create a new issue.
- Know how to write a good issue: Read e.g. <https://wiredcraft.com/blog/how-we-write-our-github-issues/> (also applies pretty much got GitLab)

## Write Code

If you feel comfortable enough writing code, you can also submit your changes directly via a merge request. Here are a few pointers that might help write the code:

## Compilation

### GNU/Linux

Compile Flare using the following commands

```bash
meson build -Dprofile=development   # Run once before all changes you make. Substitute "development" for "default" for compiling for release.
meson compile -C build   # Run every time you want to test your changes
GSETTINGS_SCHEMA_DIR=./build/data/ RUST_LOG=flare=trace ./build/target/debug/flare   # Run your locally compiled application with some logging
```

### Flatpak via fenv

As an alternative, [fenv](https://gitlab.gnome.org/ZanderBrown/fenv) allows to setup a flatpak
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

After that, move into the directory where you cloned Flare and setup the project:

```sh
# Setup the flatpak environment
fenv gen build-aux/de.schmidhuberj.Flare.Devel.json

# Launch a shell inside the build environment
fenv shell
```

You can now follow the compilation phase for GNU/Linux

### Some useful documentation

The following documentation might help:

- [GTK Book](https://gtk-rs.org/gtk4-rs/stable/latest/book/): General GTK-development.
- [presage](https://whisperfish.github.io/presage/presage/): The backend library used to interact with Signal.

### Things to keep in mind

- Make sure the code passes some basic checks (`cargo check`).
- Make sure the code is properly formatted (`cargo fmt`).
- Reach out to me for bigger changes, either in an issue or via Matrix.
- Fill out the GitLab merge request template.
- Optional but encouraged: Check for new warnings in `cargo clippy`.
- Always feel free to ask questions or request some help.
