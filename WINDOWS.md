# Building Flare on Windows: A Comprehensive Guide

## Table of Contents
1. Prerequisites
2. Installing Dependencies
3. Setting Up the Build Environment
4. Configuring the Makefile for Windows
5. Windows Credential Manager Integration
6. Bundling Required DLLs
7. GSettings Schemas for Windows
8. Database Location and Management
9. Troubleshooting Common Issues

---

## Prerequisites

- MSYS2 MinGW 64-bit: A development environment for Windows
- Git: To clone the Flare repository
- Rust: Installed via https://rustup.rs/
- Meson: Build system for Flare
- Cargo: Rust's package manager
- GTK4 and its dependencies: For GUI support
- SQLite: For the database
- OpenSSL: For encryption

---

## Installing Dependencies

### 1. Install MSYS2 MinGW 64-bit
Download and install MSYS2 from https://www.msys2.org/

### 2. Update MSYS2 Packages
Open MSYS2 MinGW 64-bit terminal and run:
pacman -Syu

### 3. Install Required Packages
pacman -S --needed base-devel mingw-w64-x86_64-toolchain mingw-w64-x86_64-meson mingw-w64-x86_64-gtk4 mingw-w64-x86_64-libadwaita mingw-w64-x86_64-gdk-pixbuf2 mingw-w64-x86_64-openssl

### 4. Install make
pacman -S make

### 5. Install Rust
Follow the instructions on https://rustup.rs/

---

## Setting Up the Build Environment

### 1. Clone the Flare Repository
git clone https://github.com/jsiulian/Flare.git
cd Flare

### 2. Switch to the windows Branch
git checkout windows

### 3. Update Dependencies
cargo update

---

## Configuring the Makefile for Windows

### Add Windows Build Targets

Modify the Makefile to include Windows-specific targets:

.PHONY: windows windows-gtk windows-gtk-run

check-deps-windows:
	@command -v meson >/dev/null 2>&1 || { echo "ERROR: meson not found. Install: pacman -S mingw-w64-x86_64-meson"; exit 1; }
	@command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found. Install Rust: https://rustup.rs"; exit 1; }
	@pkg-config --exists gtk4 2>/dev/null || { echo "ERROR: gtk4 not found. Install: pacman -S mingw-w64-x86_64-gtk4"; exit 1; }
	@pkg-config --exists libadwaita-1 2>/dev/null || { echo "ERROR: libadwaita not found. Install: pacman -S mingw-w64-x86_64-libadwaita"; exit 1; }
	@pkg-config --exists gdk-pixbuf-2.0 2>/dev/null || { echo "ERROR: gdk-pixbuf-2.0 not found. Install: pacman -S mingw-w64-x86_64-gdk-pixbuf2"; exit 1; }

windows: check-deps-windows
	@echo "Building for Windows..."
	meson setup build-windows --prefix=/mingw64
	meson compile -C build-windows

windows-gtk: check-deps-windows
	@echo "Building for Windows GTK..."
	meson setup build-windows-gtk --prefix=/mingw64 -Dprofile=default
	meson compile -C build-windows-gtk

windows-gtk-run: windows-gtk
	@echo "Running Flare..."
	GSETTINGS_SCHEMA_DIR=$(CURDIR)/build-windows-gtk/data/ RUST_LOG=flare=trace PATH=/mingw64/bin:$$PATH $(CURDIR)/build-windows-gtk/target/release/flare.exe

clean:
	rm -rf build-windows build-windows-gtk

---

## Windows Credential Manager Integration

### Add Windows Implementation for encryption_password

Add this to src/backend/manager.rs:

#[cfg(target_os = "windows")]
async fn encryption_password() -> Result<String, ApplicationError> {
    use windows::{
        core::*,
        Win32::{
            Foundation::*,
            Security::Credentials::*,
        },
    };

    let target_name = "Flare: Encryption password";
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();

    unsafe {
        if CredReadW(
            PCWSTR::from_raw(target_name.encode_utf16().chain(Some(0)).collect::<Vec<_>>().as_ptr()),
            CRED_TYPE_GENERIC.0 as u32,
            0,
            &mut credential,
        ).is_ok() {
            let secret = String::from_utf16_lossy(std::slice::from_raw_parts(
                (*credential).CredentialBlob,
                (*credential).CredentialBlobSize as usize / 2,
            ));
            CredFree(credential);
            return Ok(secret);
        }
    }

    // Create and store new credential
    let secret = rand::distr::StandardUniform {}
        .sample_string(&mut rand::rng(), SECRET_LENGTH);
    let secret_bytes = secret.as_bytes();

    let credential = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PCWSTR::from_raw(
            target_name
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>()
                .as_ptr(),
        ),
        Comment: PCWSTR::null(),
        LastWritten: FILETIME::default(),
        CredentialBlobSize: secret_bytes.len() as u32,
        CredentialBlob: secret_bytes.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_ENTERPRISE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: PCWSTR::null(),
        UserName: PCWSTR::null(),
    };

    unsafe {
        if CredWriteW(&credential, 0).is_ok() {
            Ok(secret)
        } else {
            Err(ApplicationError::ConfigurationError(
                crate::ConfigurationError::KeychainError(std::io::Error::last_os_error()),
            ))
        }
    }
}

### Add Dependency to Cargo.toml

[target.'cfg(windows)'.dependencies]
windows = { version = "0.51", features = ["Win32_Foundation", "Win32_Security_Credentials"] }

---

## Bundling Required DLLs

### Required DLLs
- libcrypto-3-x64.dll
- libcairo-2.dll
- libadwaita-1-0.dll
- libgdk_pixbuf-2.0-0.dll

### Steps to Bundle
1. Navigate to C:\msys64\mingw64\bin\
2. Copy the above DLLs to the directory where flare.exe is located (e.g., D:\Projects\Flare\build\target\release\)

---

## GSettings Schemas for Windows

### Steps to Bundle GSettings Schemas
1. Navigate to C:\msys64\mingw64\share\glib-2.0\schemas\
2. Copy gschemas.compiled to D:\Projects\Flare\build\target\release\data\schemas\.

### Set Environment Variable
Use a batch file to set the GSETTINGS_SCHEMA_DIR:

@echo off
set HERE=%~dp0
set GSETTINGS_SCHEMA_DIR=%HERE%data\schemas\
"%HERE%flare.exe" %*

---

## Database Location and Management

### Database Location
- Path: %APPDATA%\Roaming\db.sqlite
  - Full path: C:\Users\<YourUsername>\AppData\Roaming\db.sqlite

### Steps to Reset the Database
1. Navigate to %APPDATA%\Roaming\
2. Delete db.sqlite (and any associated .shm or .wal files if present)
3. Restart Flare to recreate the database

---

## Troubleshooting Common Issues

### 1. Missing decode_pixbuf in blurhash
- Error: cannot find function 'decode_pixbuf' in crate 'blurhash'
- Solution: Add the gdk-pixbuf feature to the blurhash dependency in Cargo.toml:
  [dependencies]
  blurhash = { version = "0.2", features = ["gdk-pixbuf"] }

### 2. encryption_password Not Found
- Error: cannot find function 'encryption_password' in this scope
- Solution: Ensure the Windows implementation is added to src/backend/manager.rs and the windows feature is enabled

### 3. SQLite Database Errors
- Error: file is not a database
- Solution: Delete the corrupted db.sqlite file and restart Flare

### 4. Missing GSettings Schema
- Error: Settings schema 'de.schmidhuberj.Flare' is not installed
- Solution: Ensure gschemas.compiled is in the correct location and GSETTINGS_SCHEMA_DIR is set

### 5. DLL Not Found
- Error: DLL not found dialogs
- Solution: Copy all required DLLs to the directory containing flare.exe

---

## Final Notes

- Clean Build: Always run make clean before rebuilding to avoid stale artifacts
- Logging: Use RUST_LOG=flare=trace to enable detailed logging
- Testing: Test on a clean Windows VM or a fresh user profile to ensure all dependencies are correctly installed

---
Last Updated: April 2026
