# 0009 — Seeded preset ordering

- **Milestone:** M1
- **Depends on:** 0008
- **Files:** `src/presets.rs`
- **Feature:** default

## Goal

Shuffle the scanned preset list with a deterministic RNG so that the same
`--seed` always produces the same preset order.

## Rules

- KISS: use `rand::rngs::StdRng` and `SliceRandom::shuffle`.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add to `src/presets.rs`:

```rust
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

/// Shuffle `presets` deterministically for a given seed.
///
/// The result is always a permutation of the input.
pub fn shuffle(mut presets: Vec<PathBuf>, seed: u64) -> Vec<PathBuf> {
    let mut rng = StdRng::seed_from_u64(seed);
    presets.shuffle(&mut rng);
    presets
}
```

Do not shuffle an empty list specially; `shuffle` handles it. The caller
(`scan`) already guarantees at least one preset.

## Tests

Add to the `tests` module in `src/presets.rs`:

```rust
fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn same_seed_produces_same_order() {
    let input = paths(&["a.milk", "b.milk", "c.milk", "d.milk", "e.milk"]);
    let first = shuffle(input.clone(), 42);
    let second = shuffle(input, 42);
    assert_eq!(first, second);
}

#[test]
fn shuffle_is_a_permutation() {
    let mut input = paths(&["a.milk", "b.milk", "c.milk", "d.milk"]);
    let shuffled = shuffle(input.clone(), 7);

    input.sort();
    let mut shuffled_sorted = shuffled.clone();
    shuffled_sorted.sort();

    assert_eq!(shuffled_sorted, input);
    assert_eq!(shuffled.len(), input.len());
}

#[test]
fn different_seeds_usually_differ() {
    let input = paths(&[
        "a.milk", "b.milk", "c.milk", "d.milk", "e.milk", "f.milk", "g.milk", "h.milk",
    ]);
    let a = shuffle(input.clone(), 1);
    let b = shuffle(input, 2);
    assert_ne!(a, b, "two seeds produced the same order; check the RNG setup");
}

#[test]
fn empty_list_stays_empty() {
    assert!(shuffle(Vec::new(), 123).is_empty());
}

#[test]
fn single_preset_stays_first() {
    let input = paths(&["only.milk"]);
    assert_eq!(shuffle(input.clone(), 99), input);
}
```

`different_seeds_usually_differ` is deterministic for a fixed `rand` version.
If `rand` is upgraded and the test fails, do not weaken it silently; check that
the shuffle is still seeded and update the test with the new known order if
necessary.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets presets
cargo test --all-targets
```

## Acceptance criteria

- [x] `shuffle` returns a permutation of its input.
- [x] The same seed returns the same order.
- [x] Different seeds can return different orders.
- [x] Empty and single-element inputs are handled.
- [x] No global RNG state is used.

## Notes for the implementer

- `StdRng` is not guaranteed to be stable across major `rand` versions. That is
  acceptable: determinism is required within a build, not across dependency
  upgrades. Document this in the README if it matters to users.
- Do not use `thread_rng()` here. The whole point is reproducibility.
- The default seed (when `--seed` is omitted) is resolved in ticket `0016`.
