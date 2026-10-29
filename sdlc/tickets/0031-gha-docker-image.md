# 0031 — Build and publish the Docker image in GitHub Actions

- **Milestone:** M4 / distribution
- **Depends on:** 0026, 0030
- **Files:** `.github/workflows/ci.yml`, `README.md`, `docs/release-checklist.md`
- **Feature:** `projectm` for the image; default for the fast CI job

## Goal

Make CI produce the container image that ticket `0026` defined. Every push and
pull request that passes the fast and native jobs builds the image and
smoke-tests the binary inside it. Pushes to the default branch and `v*` tags
publish the image to GitHub Container Registry (GHCR). Pull requests and feature
branches must never push.

The existing `Dockerfile` already contains `pmvid`, the projectM shared
libraries, ffmpeg, and Mesa. Do not change it in this ticket unless the image
build is actually broken.

## Rules

- KISS: add one job to the existing `.github/workflows/ci.yml`; do not create a
  second workflow.
- Do not add repository secrets. Use the workflow `GITHUB_TOKEN` with
  `packages: write`. The only manual setup is enabling package write permission
  for Actions if the repository/org disables it.
- Build `linux/amd64` only. The project targets Linux x86_64; do not add QEMU or
  multi-arch builds in this ticket.
- Never publish from a pull request, a fork, or a non-default branch.
- Use the existing `Dockerfile` and `.dockerignore` unchanged.
- Docker/CI checks are manual. The Rust test suite must remain green.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test --all-targets` before finishing, even though this ticket is
  mostly YAML.

## Implementation

### 1. Add a `docker` job to `.github/workflows/ci.yml`

Append this job under `jobs:`, after `fast` and `native`. Keep the existing jobs
and triggers unchanged.

```yaml
  docker:
    name: docker image
    runs-on: ubuntu-latest
    needs: [fast, native]
    timeout-minutes: 90
    permissions:
      contents: read
      packages: write
    env:
      # Publish only for the default branch and version tags. Keep this a string
      # so the step conditions can compare it consistently.
      PUSH_IMAGE: ${{ github.event_name == 'push' && (github.ref == format('refs/heads/{0}', github.event.repository.default_branch) || startsWith(github.ref, 'refs/tags/v')) && 'true' || 'false' }}
    steps:
      - uses: actions/checkout@v4

      # GHCR requires a lowercase image name. github.repository keeps the
      # owner's display case, so "Antervel/pmvid" becomes "ghcr.io/antervel/pmvid".
      - name: Compute image name
        run: echo "IMAGE_NAME=ghcr.io/${GITHUB_REPOSITORY,,}" >> "$GITHUB_ENV"

      - uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        if: env.PUSH_IMAGE == 'true'
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Image metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.IMAGE_NAME }}
          tags: |
            type=ref,event=branch
            type=ref,event=tag
            type=ref,event=pr
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=sha,prefix=sha-
            type=raw,value=latest,enable={{is_default_branch}}

      - name: Build image
        uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64
          push: ${{ env.PUSH_IMAGE == 'true' }}
          load: ${{ env.PUSH_IMAGE != 'true' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Smoke test image
        run: |
          image="$(printf '%s\n' '${{ steps.meta.outputs.tags }}' | head -n1)"
          docker run --rm "$image" --version
          docker run --rm "$image" --help >/dev/null
```

Why the pieces are shaped this way:

- `needs: [fast, native]` prevents a broken revision from producing an image.
- `load` and `push` are mutually exclusive in `docker/build-push-action`. The job
  loads the image for PR/feature-branch runs and only pushes for default-branch
  and `v*` tag runs.
- The smoke test works in both modes: a loaded local image is run directly; a
  pushed image is pulled from GHCR while the login is still active.
- `type=raw,value=latest,enable={{is_default_branch}}` makes `latest` point at
  the default branch, not at an old release tag.
- `type=semver` turns a tag like `v0.1.0` into `0.1.0` and `0.1`. The plain
  `ref` tag (`v0.1.0`) and a `sha-...` tag are published too.
- `cache-from`/`cache-to` use the GitHub Actions cache. The first projectM build
  is still slow; later runs reuse the Docker layers.

Do not use `pull_request_target`. The workflow uses `pull_request`, so fork PRs
build with a read-only token and the login/push steps are skipped.

If the repository or organization restricts package creation, document the fix
in the PR: enable "Read and write permissions" for the workflow, or allow the
`pmvid` package to be created by Actions. Do not add a PAT.

### 2. Document the image in `README.md`

Add a short section after **Install**:

````markdown
## Docker image

CI publishes the image to GHCR when the default branch or a `v*` tag is pushed.
Pull requests build and smoke-test the image but do not publish it.

```sh
docker pull ghcr.io/antervel/pmvid:latest
docker run --rm -v "$PWD:/work" ghcr.io/antervel/pmvid:latest \
  -i song.mp3 -o song.mp4 --presets /work/presets
```

The preset directory must be mounted too; presets are not bundled. The image
runs headless with Mesa software rendering (`LIBGL_ALWAYS_SOFTWARE=1`).

The GHCR package is private by default. Either set its visibility to public in
the package settings or run `docker login ghcr.io` with a token that can read
packages before pulling.
````

If the repository owner changes, update the image name to the lowercase
`ghcr.io/<owner>/pmvid`; GHCR rejects uppercase paths.

### 3. Add release checks to `docs/release-checklist.md`

Add these items to the checklist:

```markdown
- [ ] The CI `docker` job is green and published the release tag.
- [ ] `docker pull ghcr.io/antervel/pmvid:<version>` succeeds.
- [ ] The pulled image runs `--version` and renders a short MP4 headlessly.
```

Add this to the **Docker check** section:

```sh
docker pull ghcr.io/antervel/pmvid:latest
docker run --rm ghcr.io/antervel/pmvid:latest --version
```

## Tests to run

Fast suite (the YAML change must not break the Rust checks):

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Local image build (same as `0026`, but confirms the image this job publishes):

```sh
docker build --platform linux/amd64 -t pmvid:ci .
docker run --rm pmvid:ci --version
docker run --rm pmvid:ci --help
docker run --rm --entrypoint sh pmvid:ci -c \
  'ldd /usr/local/bin/pmvid | grep projectM'
```

On an aarch64 development machine, `--platform linux/amd64` needs QEMU/binfmt.
If emulation is unavailable, fall back to `docker build -t pmvid:ci .` for a
native smoke test and review the CI run for the actual `linux/amd64` image.

Local headless render:

```sh
tmp="$(mktemp -d)"
mkdir -p "$tmp/presets"
cp tests/fixtures/simple.milk "$tmp/presets/"
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=2" "$tmp/tone.mp3"
docker run --rm -v "$tmp:/work" pmvid:ci \
  -i /work/tone.mp3 -o /work/tone.mp4 --presets /work/presets --seed 42
ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$tmp/tone.mp4"
```

Expected: `h264` and `aac`, with no missing-library error.

Workflow validation (after pushing the branch or opening a PR):

1. The `docker` job runs only after `fast` and `native` pass.
2. A pull request builds and smoke-tests the image but does not push. The job log
   must not contain a GHCR login or push.
3. A push to the default branch pushes `main`, `latest`, and `sha-...`.
4. A `v0.1.0` tag pushes `v0.1.0`, `0.1.0`, `0.1`, and `sha-...`.
5. In the repository package settings, the `pmvid` package exists and points at
   this repository.

Verify a published image from another directory:

```sh
docker pull ghcr.io/antervel/pmvid:latest
docker run --rm ghcr.io/antervel/pmvid:latest --version
docker image inspect ghcr.io/antervel/pmvid:latest \
  --format '{{.Config.Entrypoint}} {{.Config.WorkingDir}}'
```

## Acceptance criteria

- [x] `.github/workflows/ci.yml` has a `docker` job with `needs: [fast, native]`
      and a 90-minute timeout.
- [x] The job requests `contents: read` and `packages: write` and uses
      `secrets.GITHUB_TOKEN`; no new repository secret is required.
- [x] Pull requests and feature branches build and smoke-test the image without
      logging in to GHCR or pushing.
- [x] Pushes to the default branch publish `latest` and a branch/`sha` tag.
- [x] `v*` tag pushes publish semver tags (`0.1.0`, `0.1`) plus the ref and `sha`
      tags.
- [x] The image name is lowercased, so the current `Antervel/pmvid` repository
      publishes to `ghcr.io/antervel/pmvid`.
- [x] The image is `linux/amd64` only.
- [x] The `--version` and `--help` smoke tests pass inside the published image.
- [x] The local headless render produces an H.264/AAC MP4.
- [x] `README.md` documents pulling and running the image.
- [x] The GHCR package is public, or `README.md` explains how to log in for a
      private package.
- [x] `docs/release-checklist.md` includes the GHCR pull and render checks.
- [x] Fast and native CI jobs are unchanged and green.

## Notes for the implementer

- The first Docker build is slow because projectM is compiled from source and
  downloaded from GitHub. Do not add a `timeout-minutes` below 60.
- Do not publish an image from an untrusted PR. The `PUSH_IMAGE` expression and
  the `pull_request` trigger are what keep this safe; do not replace them with
  `pull_request_target` or an unconditional `push: true`.
- `docker run` on a pushed tag will pull from GHCR. If it fails with
  `unauthorized`, the login step ran too late or `packages: write` is missing.
- If the image name contains uppercase letters, Docker fails with
  `invalid reference format`; use the `${GITHUB_REPOSITORY,,}` step, do not
  hardcode `github.repository` in `images:`.
- GHCR packages are private by default. The workflow cannot change package
  visibility; either make the package public in its settings or document the
  `docker login ghcr.io` step. Do not add a personal access token just to make
  anonymous pulls work.
- Do not bake presets into the image and do not add a render step that needs a
  host `ffmpeg`; the image contains ffmpeg, and presets are mounted at runtime.
- Do not add image signing, SBOM attestations, or multi-arch builds in this
  ticket. They are separate work.
- If the `docker` job cannot push because package creation is disabled, stop and
  report the repository setting; do not add a personal access token.
