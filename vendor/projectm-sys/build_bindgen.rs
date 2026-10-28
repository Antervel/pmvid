use std::env;
use std::path::{Path, PathBuf};

pub fn bindgen(include_dir: &Path) {
    println!("cargo:rerun-if-changed=bindgen/default.h");
    println!("cargo:rerun-if-changed=bindgen/playlist.h");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let out_path = out_dir.join("bindings.rs");
    let header = if cfg!(feature = "playlist") {
        "bindgen/playlist.h"
    } else {
        "bindgen/default.h"
    };

    let bindings = bindgen::Builder::default()
        .header(header)
        .allowlist_function("projectm_.*")
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("unable to generate projectM bindings");

    bindings.write_to_file(out_path).expect("could not write projectM bindings");
}