#!/usr/bin/env bash
export DIST="$1"
export SOURCE_ROOT="$2"

cd "$SOURCE_ROOT"
mkdir "$DIST"/.cargo
cargo vendor | sed 's/^directory = ".*"/directory = "vendor"/g' > $DIST/.cargo/config
# Move vendor into dist tarball directory
mv vendor "$DIST"

# presage-store-sqlite requires .sqlx folder to compile.
cp -r $MESON_BUILD_ROOT/cargo-home/git/checkouts/presage*/*/.sqlx "$DIST/vendor/presage-store-sqlite"

echo "Finished Vendor"
