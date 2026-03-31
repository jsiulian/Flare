# Flare build wrapper
#
# Usage:
#   make                  — Linux release build
#   make macos            — macOS release build (GTK)
#   make native           — macOS native cacao build (no GTK needed)
#   make windows-gtk      — Windows MinGW/MSYS2 GTK release build
#   make debug            — Linux debug build
#   make debug macos      — macOS GTK debug build
#   make debug native     — macOS native cacao debug build
#   make debug windows-gtk — Windows MinGW/MSYS2 GTK debug build
#   make run              — Linux release build + launch
#   make run macos        — macOS GTK release build + launch
#   make run native       — macOS native cacao build + launch
#   make run windows-gtk  — Windows MinGW/MSYS2 GTK release build + launch
#   make debug run        — Linux debug build + launch
#   make debug run macos  — macOS GTK debug build + launch
#   make debug run native — macOS native cacao debug build + launch
#   make debug run windows-gtk — Windows GTK debug build + launch
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
RECONFIGURE      = $(if $(wildcard $(BUILD_DIR)/build.n
