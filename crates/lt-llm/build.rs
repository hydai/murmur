use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Only build the Swift bridge on macOS.
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let swift_package_dir = manifest_dir.parent().unwrap().join("lt-llm-apple");

    // Skip if the Swift package directory doesn't exist (e.g. in CI without submodule).
    if !swift_package_dir.exists() {
        println!("cargo:warning=lt-llm-apple not found, skipping Apple LLM bridge");
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
    println!(
        "cargo:rustc-link-search=native={}",
        swift_build_dir.display()
    );
    println!("cargo:rustc-link-lib=static=LlmBridge");

    // Link Apple frameworks required by the Swift bridge.
    println!("cargo:rustc-link-lib=framework=Foundation");

    // Link the Swift runtime libraries.
    // On macOS 26+, these live in /usr/lib/swift/ (dyld shared cache).
    // The toolchain's lib/swift/macosx/ contains static compatibility stubs.
    if let Some(toolchain_dir) = get_swift_toolchain_lib_dir() {
        println!("cargo:rustc-link-search=native={}", toolchain_dir);
    }

    // Link Swift runtime dylibs from system.
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-lib=dylib=swiftFoundation");
    println!("cargo:rustc-link-lib=dylib=swiftDispatch");

    // Set rpath for test binaries so they can find Swift runtime dylibs.
    // The final app binary gets its rpath from lt-tauri/build.rs instead.
    println!("cargo:rustc-link-arg-tests=-Wl,-rpath,/usr/lib/swift");
}

/// Find the Swift toolchain library directory (contains .a compatibility stubs).
fn get_swift_toolchain_lib_dir() -> Option<String> {
    let output = Command::new("xcrun")
        .args(["--toolchain", "default", "--find", "swift"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let swift_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let lib_dir = PathBuf::from(&swift_path)
        .parent()?
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
