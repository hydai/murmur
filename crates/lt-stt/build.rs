use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Only build the Swift bridge on macOS.
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let swift_package_dir = manifest_dir.parent().unwrap().join("lt-stt-apple");

    // Skip if the Swift package directory doesn't exist (e.g. in CI without submodule).
    if !swift_package_dir.exists() {
        println!("cargo:warning=lt-stt-apple not found, skipping Apple STT bridge");
        return;
    }

    // Rebuild when Swift sources change.
    println!(
        "cargo:rerun-if-changed={}",
        swift_package_dir.join("Sources").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        swift_package_dir.join("Package.swift").display()
    );

    // Run swift build.
    let output = Command::new("swift")
        .args(["build", "-c", "release"])
        .arg("--package-path")
        .arg(&swift_package_dir)
        .output()
        .expect("Failed to execute swift build. Is Xcode installed?");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("swift build failed:\n{}", stderr);
    }

    // Find the built static library.
    // Swift PM puts release artifacts under .build/release/
    let swift_build_dir = swift_package_dir.join(".build").join("release");
    println!("cargo:rustc-link-search=native={}", swift_build_dir.display());
    println!("cargo:rustc-link-lib=static=SpeechBridge");

    // Link Apple frameworks required by the Swift bridge.
    println!("cargo:rustc-link-lib=framework=Speech");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreMedia");

    // Link the Swift runtime and standard library.
    // Find the Swift lib directory from the toolchain.
    if let Some(dir) = get_swift_lib_dir() {
        println!("cargo:rustc-link-search=native={}", dir);
    }

    // The Swift static library needs these runtime libraries.
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-lib=dylib=swift_Concurrency");
    println!("cargo:rustc-link-lib=dylib=swift_StringProcessing");
    println!("cargo:rustc-link-lib=dylib=swiftFoundation");
    println!("cargo:rustc-link-lib=dylib=swiftDispatch");
}

/// Find the Swift runtime library directory.
fn get_swift_lib_dir() -> Option<String> {
    // Try xcrun to find the toolchain.
    let output = Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // The Swift runtime dylibs are typically in the toolchain's lib/swift/macosx/
    let output = Command::new("xcrun")
        .args(["--toolchain", "default", "--find", "swift"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let swift_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let swift_bin_dir = PathBuf::from(&swift_path)
        .parent()?
        .to_path_buf();
    let lib_dir = swift_bin_dir
        .parent()?
        .join("lib")
        .join("swift")
        .join("macosx");

    if lib_dir.exists() {
        Some(lib_dir.to_string_lossy().to_string())
    } else {
        None
    }
}
