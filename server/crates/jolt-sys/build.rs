use cmake::Config;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=vendor/joltc");
    println!("cargo:rerun-if-changed=build.rs");

    // Build joltc via CMake
    let dst = Config::new("vendor/joltc")
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("JPH_BUILD_SHARED", "OFF")
        .build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=joltc");
    println!("cargo:rustc-link-lib=static=Jolt");

    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=stdc++");

    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=c++");

    // Generate Rust bindings from joltc.h
    let bindings = bindgen::Builder::default()
        .header("vendor/joltc/include/joltc.h")
        .allowlist_function("JPH_.*")
        .allowlist_type("JPH_.*")
        .allowlist_var("JPH_.*")
        .generate()
        .expect("failed to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings");
}
