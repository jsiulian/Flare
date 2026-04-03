# Flare build wrapper
#
# Usage:
#   make                  — Linux release build
#   make macos            — macOS release build (GTK)
#   make native           — macOS native cacao build (no GTK needed)
#   make windows          — Windows/MSYS2 release build (GTK)
#   make debug            — Linux debug build
#   make debug macos      — macOS GTK debug build
#   make debug native     — macOS native cacao debug build
#   make debug windows    — Windows/MSYS2 GTK debug build
#   make run              — Linux release build + launch
#   make run macos        — macOS GTK release build + launch
#   make run native       — macOS native cacao build + launch
#   make run windows      — Windows/MSYS2 release build + launch
#   make debug run        — Linux debug build + launch
#   make debug run macos  — macOS GTK debug build + launch
#   make debug run native — macOS native cacao debug build + launch
#   make debug run windows — Windows/MSYS2 GTK debug build + launch
#   make app              — macOS release build + Finder-launchable .app bundle
#   make debug app        — same with a debug binary
#   make clean            — Remove all build artifacts and logs

PROFILE   ?= default
BUILD_DIR  = build

# 'debug' sets the Meson development profile (no --release, debug binary)
ifeq ($(filter debug,$(MAKECMDGOALS)),debug)
override PROFILE = development
endif

# Rust target dir mirrors the profile
ifeq ($(PROFILE),development)
RUST_TARGET = debug
else
RUST_TARGET = release
endif

# Reconfigure an existing build dir; set up fresh otherwise
RECONFIGURE      = $(if $(wildcard $(BUILD_DIR)/build.ninja),--reconfigure,)
MESON_SETUP      = meson setup $(BUILD_DIR) $(RECONFIGURE) -Dprofile=$(PROFILE)
MESON_BUILD      = meson compile -C $(BUILD_DIR)

# macOS: gettext headers are not on the default pkg-config path
MACOS_PKG_CONFIG = PKG_CONFIG_PATH="$$(brew --prefix gettext)/lib/pkgconfig:$$PKG_CONFIG_PATH"

# Windows/MSYS2: ensure MinGW64 paths are set
WINDOWS_ENV      = PKG_CONFIG_PATH="/mingw64/lib/pkgconfig:$$PKG_CONFIG_PATH" \
                   PATH="/mingw64/bin:$$PATH"

RUN_CMD          = GSETTINGS_SCHEMA_DIR=$(BUILD_DIR)/data/ RUST_LOG=flare=trace \
                   $(BUILD_DIR)/target/$(RUST_TARGET)/flare

WINDOWS_RUN_CMD  = GSETTINGS_SCHEMA_DIR=$(BUILD_DIR)/data/ RUST_LOG=flare=trace \
                   $(BUILD_DIR)/target/$(RUST_TARGET)/flare.exe

# Native cacao GUI (macOS only)
NATIVE_CARGO_BUILD = cargo build --features cacao-gui
ifeq ($(PROFILE),development)
NATIVE_BINARY = target/debug/flare
else
NATIVE_CARGO_BUILD += --release
NATIVE_BINARY = target/release/flare
endif
NATIVE_RUN_CMD = RUST_LOG=flare=trace $(NATIVE_BINARY)

.PHONY: all linux macos native windows debug run app clean check-deps check-deps-windows

# Dependency check — runs before every build target (not clean)
check-deps:
	@command -v meson         >/dev/null 2>&1 || { echo "ERROR: meson not found. Install: pip install meson  OR  brew install meson"; exit 1; }
	@command -v cargo         >/dev/null 2>&1 || { echo "ERROR: cargo not found. Install Rust: https://rustup.rs"; exit 1; }
	@command -v blueprint-compiler >/dev/null 2>&1 || { echo "ERROR: blueprint-compiler not found. Install: pip install blueprint-compiler  OR  brew install blueprint-compiler"; exit 1; }
	@pkg-config --exists gtk4 2>/dev/null        || { echo "ERROR: gtk4 not found. Install: brew install gtk4"; exit 1; }
	@pkg-config --exists libadwaita-1 2>/dev/null || { echo "ERROR: libadwaita not found. Install: brew install libadwaita"; exit 1; }
	@{ [ -d "$$(pkg-config --variable=iconsdir gtk4 2>/dev/null)/Adwaita" ] || \
	   [ -d "/usr/share/icons/Adwaita" ] || \
	   [ -d "/opt/homebrew/share/icons/Adwaita" ] || \
	   [ -d "$$HOME/.local/share/icons/Adwaita" ]; } || \
	  { echo "ERROR: adwaita-icon-theme not found. Install: brew install adwaita-icon-theme  OR  sudo apt install adwaita-icon-theme"; exit 1; }

check-deps-windows:
	@command -v meson    >/dev/null 2>&1 || { echo "ERROR: meson not found. Run: pacman -S mingw-w64-x86_64-meson"; exit 1; }
	@command -v cargo    >/dev/null 2>&1 || { echo "ERROR: cargo not found. Run: pacman -S mingw-w64-x86_64-rust"; exit 1; }
	@command -v blueprint-compiler >/dev/null 2>&1 || { echo "ERROR: blueprint-compiler not found. Run: pacman -S mingw-w64-x86_64-blueprint-compiler"; exit 1; }
	@pkg-config --exists gtk4         2>/dev/null || { echo "ERROR: gtk4 not found. Run: pacman -S mingw-w64-x86_64-gtk4"; exit 1; }
	@pkg-config --exists libadwaita-1 2>/dev/null || { echo "ERROR: libadwaita not found. Run: pacman -S mingw-w64-x86_64-libadwaita"; exit 1; }
	@pkg-config --exists gtksourceview-5 2>/dev/null || { echo "ERROR: gtksourceview-5 not found. Run: pacman -S mingw-w64-x86_64-gtksourceview5"; exit 1; }

all: check-deps linux

# linux/macos/windows skip their build when 'run' is in goals — run handles it
linux: check-deps
ifeq ($(filter run,$(MAKECMDGOALS)),)
	$(MESON_SETUP)
	$(MESON_BUILD)
else
	@:
endif

macos: check-deps
ifeq ($(filter run,$(MAKECMDGOALS)),)
	$(MACOS_PKG_CONFIG) $(MESON_SETUP)
	$(MACOS_PKG_CONFIG) $(MESON_BUILD)
else
	@:
endif

windows: check-deps-windows
ifeq ($(filter run,$(MAKECMDGOALS)),)
	$(WINDOWS_ENV) $(MESON_SETUP)
	$(WINDOWS_ENV) $(MESON_BUILD)
else
	@:
endif

# Native cacao build (macOS only, no GTK/meson dependency)
native: check-deps-native
ifeq ($(filter run,$(MAKECMDGOALS)),)
	$(NATIVE_CARGO_BUILD)
else
	@:
endif

check-deps-native:
	@command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found. Install Rust: https://rustup.rs"; exit 1; }

# Modifier target — no-op when a platform or 'run' is also specified
debug: check-deps
# Treat `app` and `native` as platform goals too so `make debug app` doesn't
# fall back to the linux build path.
ifeq ($(filter linux macos native windows run app,$(MAKECMDGOALS)),)
	$(MAKE) --no-print-directory linux PROFILE=$(PROFILE)
else
	@:
endif

# Build and launch — picks platform from goals, profile from debug modifier
run:
ifeq ($(filter native,$(MAKECMDGOALS)),native)
	$(NATIVE_CARGO_BUILD)
	$(NATIVE_RUN_CMD)
else ifeq ($(filter macos,$(MAKECMDGOALS)),macos)
	$(MACOS_PKG_CONFIG) $(MESON_SETUP)
	$(MACOS_PKG_CONFIG) $(MESON_BUILD)
	$(RUN_CMD)
else ifeq ($(filter windows,$(MAKECMDGOALS)),windows)
	$(WINDOWS_ENV) $(MESON_SETUP)
	$(WINDOWS_ENV) $(MESON_BUILD)
	$(WINDOWS_RUN_CMD)
else
	$(MESON_SETUP)
	$(MESON_BUILD)
	$(RUN_CMD)
endif

# macOS application bundle target (Finder-openable .app)
# Builds with the current `PROFILE` (set by `debug` modifier when present).
app: check-deps
ifeq ($(filter run,$(MAKECMDGOALS)),)
	$(MACOS_PKG_CONFIG) $(MESON_SETUP)
	$(MACOS_PKG_CONFIG) $(MESON_BUILD)
else
	@:
endif

	@echo "Creating macOS .app bundle in $(BUILD_DIR)/Flare.app"
	mkdir -p $(BUILD_DIR)/Flare.app/Contents/MacOS
	mkdir -p $(BUILD_DIR)/Flare.app/Contents/Resources
	mkdir -p $(BUILD_DIR)/Flare.app/Contents/Resources/data
	# Copy compiled resources (schemas, gresources, etc.) into the bundle
	cp -a $(BUILD_DIR)/data/. $(BUILD_DIR)/Flare.app/Contents/Resources/data/ 2>/dev/null || true
	# Copy the Rust binary as an internal binary and create a small wrapper
	cp $(BUILD_DIR)/target/$(RUST_TARGET)/flare $(BUILD_DIR)/Flare.app/Contents/MacOS/flare-bin
	chmod +x $(BUILD_DIR)/Flare.app/Contents/MacOS/flare-bin
	# Wrapper that sets necessary env vars then execs the real binary
	printf '%s\n' \
	  '#!/bin/sh' \
	  'HERE="$$(dirname "$$0")"' \
	  'export GSETTINGS_SCHEMA_DIR="$$HERE/../Resources/data"' \
	  'exec "$$HERE/flare-bin" "$$@"' \
	  > $(BUILD_DIR)/Flare.app/Contents/MacOS/flare.tmp
	mv $(BUILD_DIR)/Flare.app/Contents/MacOS/flare.tmp $(BUILD_DIR)/Flare.app/Contents/MacOS/flare
	chmod +x $(BUILD_DIR)/Flare.app/Contents/MacOS/flare
	# Write a minimal Info.plist so Finder can launch the bundle
	# Copy app icons (SVGs) into Resources. For Finder to show an icon an
	# .icns file is required; users can generate one and place it here.
	if [ -d data/icons ] ; then \
	  cp -a data/icons/. $(BUILD_DIR)/Flare.app/Contents/Resources/ 2>/dev/null || true; \
	fi

	# Generate .icns from SVGs if possible (iconutil + rsvg-convert or ImageMagick)
	if command -v iconutil >/dev/null 2>&1; then ICON_SVG=data/icons/de.schmidhuberj.Flare.svg; if [ -f "$$ICON_SVG" ]; then TMPICONSET=$$(mktemp -d /tmp/flareicon.XXXX.iconset); SIZES="16 32 128 256 512 1024"; for s in $$SIZES; do if command -v rsvg-convert >/dev/null 2>&1; then rsvg-convert -w $$s -h $$s "$$ICON_SVG" -o "$$TMPICONSET/icon_$${s}x$${s}.png" 2>/dev/null || true; elif command -v convert >/dev/null 2>&1; then convert "$$ICON_SVG" -resize $$s "$$TMPICONSET/icon_$${s}x$${s}.png" 2>/dev/null || true; fi; s2=$$(( $$s * 2 )); if command -v rsvg-convert >/dev/null 2>&1; then rsvg-convert -w $$s2 -h $$s2 "$$ICON_SVG" -o "$$TMPICONSET/icon_$${s}x$${s}@2x.png" 2>/dev/null || true; elif command -v convert >/dev/null 2>&1; then convert "$$ICON_SVG" -resize $$s2 "$$TMPICONSET/icon_$${s}x$${s}@2x.png" 2>/dev/null || true; fi; done; if ls $$TMPICONSET/* >/dev/null 2>&1; then iconutil -c icns $$TMPICONSET -o $(BUILD_DIR)/Flare.app/Contents/Resources/de.schmidhuberj.Flare.icns >/dev/null 2>&1 || true; fi; rm -rf $$TMPICONSET || true; fi; fi

	printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' \
	  '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' \
	  '<plist version="1.0"><dict>' \
	'<key>CFBundleExecutable</key><string>flare</string>' \
	'<key>CFBundleIdentifier</key><string>de.schmidhuberj.Flare</string>' \
	'<key>CFBundleName</key><string>Flare</string>' \
	'<key>CFBundlePackageType</key><string>APPL</string>' \
	'<key>CFBundleIconFile</key><string>de.schmidhuberj.Flare.icns</string>' \
	'<key>CFBundleIconName</key><string>de.schmidhuberj.Flare</string>' \
	  '</dict></plist>' > $(BUILD_DIR)/Flare.app/Contents/Info.plist

	@echo "Bundle created: $(BUILD_DIR)/Flare.app"

clean:
	rm -rf $(BUILD_DIR) builddir target
	# Remove build DB and SQLite journal files if present
	rm -f $(BUILD_DIR)/data/db.sqlite $(BUILD_DIR)/data/db.sqlite-shm $(BUILD_DIR)/data/db.sqlite-wal 2>/dev/null || true
	rm -f $(BUILD_DIR)/Flare.app/Contents/Resources/data/db.sqlite $(BUILD_DIR)/Flare.app/Contents/Resources/data/db.sqlite-shm $(BUILD_DIR)/Flare.app/Contents/Resources/data/db.sqlite-wal 2>/dev/null || true
	find . -maxdepth 2 -name '*.log' -delete 2>/dev/null; true
