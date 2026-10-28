use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod build_bindgen;

const PROJECTM_REPO: &str = "https://github.com/projectM-visualizer/projectm.git";
const PROJECTM_COMMIT: &str = "349b7cc2db312d47cef6be24c6856ced6643a385";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PMVID_PROJECTM_SOURCE_DIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let source = projectm_source(&out_dir);

    let dst = cmake::Config::new(&source)
        .define("ENABLE_PLAYLIST", "ON")
        .define("ENABLE_INSTALL", "ON")
        .define("ENABLE_DEBUG_POSTFIX", "OFF")
        .define("ENABLE_SDL_UI", "OFF")
        .define("ENABLE_SYSTEM_PROJECTM_EVAL", "OFF")
        .build();

    // GNUInstallDirs may use lib or lib64 depending on the distribution, and
    // cmake-rs installs into OUT_DIR by default.
    for dir in ["lib", "lib64"] {
        for root in [&dst, &out_dir] {
            let path = root.join(dir);
            if path.is_dir() {
                println!("cargo:rustc-link-search=native={}", path.display());
            }
        }
    }
    println!("cargo:rustc-link-lib=dylib=projectM-4");
    println!("cargo:rustc-link-lib=dylib=projectM-4-playlist");

    // Linux: link stdc++ and GL.
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=GL");
    println!("cargo:rustc-link-lib=dylib=gomp");

    let include_dir = [dst.join("include"), out_dir.join("include")]
        .into_iter()
        .find(|path| path.is_dir())
        .expect("projectM install include directory is missing");
    build_bindgen::bindgen(&include_dir);
}

/// Return a projectM source tree, either from the environment or a pinned
/// checkout in `OUT_DIR`.
fn projectm_source(out_dir: &Path) -> PathBuf {
    if let Some(path) = env::var_os("PMVID_PROJECTM_SOURCE_DIR") {
        let path = PathBuf::from(path);
        assert!(
            path.join("CMakeLists.txt").is_file(),
            "PMVID_PROJECTM_SOURCE_DIR must point at a projectM source checkout"
        );
        assert!(
            path.join("vendor/projectm-eval/CMakeLists.txt").is_file(),
            "projectM submodules are missing; run: git -C {} submodule update --init --recursive",
            path.display()
        );
        return path;
    }

    let source = out_dir.join("projectm");
    if source.join("CMakeLists.txt").is_file() {
        if !source.join("vendor/projectm-eval/CMakeLists.txt").is_file() {
            let mut command = Command::new("git");
            command.arg("-C").arg(&source).args(["submodule", "update", "--init", "--recursive", "--depth=1"]);
            run(&mut command);
        }
        return source;
    }

    // A previous failed clone can leave a partial directory behind.
    if source.exists() {
        fs::remove_dir_all(&source).expect("failed to remove incomplete projectM checkout");
    }

    let mut command = Command::new("git");
    command
        .arg("clone")
        .arg("--filter=blob:none")
        .arg("--no-checkout")
        .arg(PROJECTM_REPO)
        .arg(&source);
    run(&mut command);

    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&source)
        .args(["fetch", "--depth=1", "origin", PROJECTM_COMMIT]);
    run(&mut command);

    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&source)
        .args(["checkout", "--detach", "FETCH_HEAD"]);
    run(&mut command);

    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&source)
        .args(["submodule", "update", "--init", "--recursive", "--depth=1"]);
    run(&mut command);

    source
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .expect("failed to run git; install git or set PMVID_PROJECTM_SOURCE_DIR");
    assert!(status.success(), "git command failed: {command:?}");
}