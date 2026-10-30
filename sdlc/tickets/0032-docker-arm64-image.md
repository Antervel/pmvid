# 0032 — Build and publish the linux/arm64 Docker image in GitHub Actions

- **Milestone:** M4 / distribution
- **Depends on:** 0026, 0031
- **Files:** `.github/workflows/ci.yml`, `README.md`, `docs/release-checklist.md`
- **Feature:** `projectm` for the image; default for the fast CI job

## Goal

Build and publish a container image for `linux/arm64` (variant `v8`, OS `linux`)
that mirrors the `linux/amd64` image from ticket `0031`.

The `0031` image is the release artifact for amd64. This ticket produces the
arm64 twin: same pmvid binary, same projectM shared libraries, same ffmpeg/Mesa
payload, different target architecture.

## Rules

- KISS: mirror the `docker` job from ticket `0031`; do not redesign the image.
- Build `linux/arm64` only in this ticket; do not add multi-arch manifests yet.
- Do not change the `Dockerfile` or `.dockerignore` unless the arm64 build is
  actually broken.
- Never publish from a pull request, fork, or non-default branch.
- No new repository secrets; reuse the workflow `GITHUB_TOKEN` with
  `packages: write`.

## Implementation

### 1. Add the arm64 image job to `.github/workflows/ci.yml`

Mirror the `0031` `docker` job, but build and smoke-test for `linux/arm64`
(variant `v8`).

- Runner choice (pick one, KISS): native GitHub arm64 runner
  (`ubuntu-24.04-arm`), or `docker/setup-qemu-action` + `docker/build-push-action`
  with `platforms: linux/arm64`.
- Keep the same gate style as the `0031` job (`needs: [fast, native]`, 90-minute
  timeout).

### 2. Publish to GHCR

Reuse the same `PUSH_IMAGE` gating, lowercase image name, and metadata tags as
the `0031` job. Publish only for the default branch and `v*` tags.

### 3. Docs

- `README.md`: arm64 pull/run commands.
- `docs/release-checklist.md`: arm64 checks.

## Tests to run

Fast suite (the change is mostly YAML):

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Local arm64 smoke test (native runner or QEMU):

```sh
docker build --platform linux/arm64 -t pmvid:arm64 .
docker run --rm pmvid:arm64 --version
docker run --rm pmvid:arm64 --help >/dev/null
```

Local headless render on arm64 (if emulation/native runner allows):

```sh
tmp="$(mktemp -d)"
mkdir -p "$tmp/presets"
cp tests/fixtures/simple.milk "$tmp/presets/"
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=2" "$tmp/tone.mp3"
docker run --rm -v "$tmp:/work" pmvid:arm64 \
  -i /work/tone.mp3 -o /work/tone.mp4 --presets /work/presets --seed 42
ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$tmp/tone.mp4"
```

## Acceptance criteria

- [x] `.github/workflows/ci.yml` builds a `linux/arm64` (variant `v8`) image.
- [x] The build mirrors the `0031` amd64 image (same Dockerfile content).
- [x] The job gates on the fast and native jobs like the `0031` job.
- [x] The job uses `GITHUB_TOKEN` with `packages: write` and no new secret.
- [x] PRs and feature branches build and smoke-test the arm64 image without publishing.
- [x] Pushes to the default branch publish the arm64 image to GHCR.
- [x] `v*` tag pushes publish the arm64 image.
- [x] The image name is lowercased consistently with the amd64 image.
- [x] The `--version` and `--help` smoke tests pass inside the arm64 image.
- [x] The local headless render produces an H.264/AAC MP4.
- [x] `README.md` documents pulling and running the arm64 image.
- [x] Fast and native CI jobs are unchanged and green.

## Notes for the implementer

- First arm64 build is slow (projectM under QEMU); keep a generous `timeout-minutes`.
- Do not add image signing, SBOM attestations, or multi-arch builds in this ticket.

## Out of scope

- Multi-arch single-manifest publishing, arm/v7, signing, SBOM, non-Linux.