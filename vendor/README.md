# Vendored dependencies

`projectm-sys` is copied from crates.io (resolved 1.2.2) and patched locally:

- the projectM source is pinned to commit
  `349b7cc2db312d47cef6be24c6856ced6643a385` (projectM v4.2.0) instead of a
  release tag;
- the CMake install include directory is passed to bindgen;
- Linux links the actual `projectM-4` and `projectM-4-playlist` libraries;
- `PMVID_PROJECTM_SOURCE_DIR` allows an offline build from an existing checkout.

`projectm` is copied from crates.io 3.1.2 and patched locally:

- C `char` pointers use `std::os::raw::c_char` instead of a hardcoded `i8`,
  so the wrapper compiles on aarch64 Linux as well as x86_64.

Do not edit these copies casually. See `sdlc/tickets/0018a-projectm-pin.md`.