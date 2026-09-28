//! Copies the license bundle next to the executable, and on a Windows HEIC
//! build the LGPL DLLs as well.
//!
//! libheif and libde265 are LGPL-3.0. They must stay shared libraries the
//! recipient can replace, with the GPL and LGPL texts beside the program.
//! `VCPKGRS_DYNAMIC` is what stops the vcpkg crate from linking them statically.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=VCPKGRS_DYNAMIC");
    println!("cargo:rerun-if-changed=LICENSE");
    println!("cargo:rerun-if-changed=NOTICE");
    println!("cargo:rerun-if-changed=THIRD_PARTY.md");
    println!("cargo:rerun-if-changed=licenses");

    let profile = profile_dir();
    copy_licenses(&profile.join("licenses"));

    if heic_enabled() && target_windows() {
        if std::env::var_os("VCPKGRS_DYNAMIC").is_none() {
            panic!(
                "HEIC on Windows must be dynamically linked so libheif and libde265 (LGPL-3.0) \
                 stay replaceable. VCPKGRS_DYNAMIC=1 is set in .cargo/config.toml; do not unset it."
            );
        }
        copy_heic_dlls(&profile);
    }
}

fn heic_enabled() -> bool {
    std::env::var_os("CARGO_FEATURE_HEIC").is_some()
}

fn target_windows() -> bool {
    std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows")
}

/// `target/<profile>` or `target/<triple>/<profile>`: the directory that contains `build/`.
fn profile_dir() -> PathBuf {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut dir = out.as_path();
    loop {
        if dir.file_name().is_some_and(|name| name == "build") {
            return dir
                .parent()
                .expect("OUT_DIR's build directory has a parent")
                .to_path_buf();
        }
        dir = dir
            .parent()
            .expect("OUT_DIR is not inside a build directory");
    }
}

fn copy_licenses(dest: &Path) {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    for name in ["LICENSE", "NOTICE", "THIRD_PARTY.md"] {
        copy_file(&root.join(name), &dest.join(name));
    }
    let licenses = root.join("licenses");
    let entries = fs::read_dir(&licenses).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", licenses.display());
    });
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|err| panic!("failed to read {}: {err}", licenses.display()));
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "txt") {
            copy_file(&path, &dest.join(entry.file_name()));
        }
    }
}

fn copy_heic_dlls(profile: &Path) {
    let bin = vcpkg_bin_dir();
    println!("cargo:rerun-if-changed={}", bin.display());
    let dlls = dlls_in(&bin);
    if dlls.is_empty() {
        panic!(
            "no HEIC DLLs in {}.\n\
             Install the dynamic triplet (the static one must not be linked):\n\
             target\\vcpkg\\vcpkg.exe install \"libheif[core]:x64-windows\"",
            bin.display()
        );
    }
    for dll in &dlls {
        let name = dll.file_name().expect("dll path has a file name");
        copy_file(dll, &profile.join(name));
        copy_file(dll, &profile.join("deps").join(name));
    }
}

fn vcpkg_bin_dir() -> PathBuf {
    let triplet = windows_triplet();
    let rel = PathBuf::from("vcpkg")
        .join("installed")
        .join(&triplet)
        .join("bin");
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target).join(&rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    manifest.join("target").join(rel)
}

fn windows_triplet() -> String {
    match std::env::var("CARGO_CFG_TARGET_ARCH").ok().as_deref() {
        Some("x86_64") => "x64-windows".to_owned(),
        Some("aarch64") => "arm64-windows".to_owned(),
        Some("x86") => "x86-windows".to_owned(),
        other => panic!("no vcpkg triplet for target arch {other:?}"),
    }
}

fn dlls_in(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|err| {
        panic!(
            "HEIC DLLs not found in {} ({err}).\n\
             Install the dynamic triplet:\n\
             target\\vcpkg\\vcpkg.exe install \"libheif[core]:x64-windows\"",
            dir.display()
        );
    });
    let mut dlls = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
        {
            dlls.push(path);
        }
    }
    dlls.sort();
    dlls
}

fn copy_file(src: &Path, dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("failed to create {}: {err}", parent.display()));
    }
    fs::copy(src, dest).unwrap_or_else(|err| {
        panic!(
            "failed to copy {} to {}: {err}",
            src.display(),
            dest.display()
        );
    });
}
