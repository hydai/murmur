fn main() {
    tauri_build::build();

    // Swift runtime rpath — must be set in the binary crate (not lt-stt library crate)
    // because cargo:rustc-link-arg only takes effect on the final linked binary.
    // On macOS with MACOSX_DEPLOYMENT_TARGET < 12.0, Apple's $ld$previous mechanism
    // rewrites Swift dylib install names from /usr/lib/swift/... to @rpath/...,
    // so we need an explicit rpath to /usr/lib/swift for dyld resolution.
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
