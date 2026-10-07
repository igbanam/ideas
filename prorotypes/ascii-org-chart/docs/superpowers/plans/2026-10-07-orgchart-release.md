# orgchart Standalone Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `orgchart` from `ideas/prorotypes/ascii-org-chart` into a new public repo `igbanam/orgchart` and release v0.1.0 on GitHub Releases, crates.io, and Homebrew (via `homebrew-servings`) with binaries for Windows, Linux, and macOS (amd64 + arm64).

**Architecture:** Fresh repo at `~/projects/igbanam/orgchart` seeded from the prototype (excluding `target/` and `.opencode/`). Two workflows: `ci.yml` (tests on 3 OSes) and `release.yml` (tag-triggered: test → 6-target build matrix → package + GitHub release → crates.io publish → formula commit to the `homebrew-servings` tap). The formula is generated from a script checked into the orgchart repo, so all three channels update from one `git tag v0.1.0 && git push origin v0.1.0`.

**Tech Stack:** Rust 1.92, GitHub Actions, `cross` (Linux aarch64 only), Homebrew Ruby formula.

**Spec:** `docs/superpowers/specs/2026-10-07-orgchart-release-design.md` (migrates with the repo in Task 1).

## Global Constraints

- Package/binary name: `orgchart` (verified free on crates.io via `cargo search`; GitHub `igbanam/orgchart` free).
- Version `0.1.0`, tag `v0.1.0`, default branch `main`, repo public.
- License: MIT, first line `Copyright (c) 2026 Owajigbanam Ogbuluijah <xigbanam@gmail.com>` (matches `reckless/LICENSE`).
- Six build targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`.
- Asset names: `orgchart-v<version>-<target>.tar.gz` (unix) / `.zip` (windows), plus one `checksums.txt` per release. Archive contains the binary at its root (`orgchart` / `orgchart.exe`).
- Formula: class `Orgchart`, tap `igbanam/homebrew-servings`, pulls binaries from `igbanam/orgchart` releases (NOT the tap's own releases).
- Secrets in `igbanam/orgchart`: `CARGO_REGISTRY_TOKEN` (crates.io), `SERVINGS_TAP_TOKEN` (PAT, repo scope on homebrew-servings). Both channels must skip-with-warning when their secret is absent — never fail the release.
- Never copy `target/` or `.opencode/` into the new repo.
- Do not change orgchart's features, CLI, or behavior.
- All commits follow conventional-commit style.

## Review Focus

Failure modes the spec implies but product tests don't cover; each pinned to its owning task:

1. **arm64 Mac install breaks** (partner's machine) — wrong aarch64 URL or sha in formula → Task 6 `brew install igbanam/servings/orgchart` + `orgchart --version` is the gate.
2. **Windows archive is a tar or missing the `.exe` suffix** → Task 5 packaging loop pins per-OS format and Task 4's windows smoke test executes the `.exe` by exact name.
3. **`cross`-built linux aarch64 binary is corrupt/unrunnable** → cannot run in CI; Task 5 verifies the archive extracts with the binary at root and checksums.txt matches `shasum` output.
4. **Release fails when a secret is missing** → Task 5 step: release job must succeed with neither secret set (first run is expected to skip crates/tap); CI gates the token-dependent jobs on `-z` checks.
5. **Ideas removal deletes the design docs** → docs/ is copied in Task 1 *before* Task 6 removes the prototype directory from `ideas`; Task 6 verifies the spec/plan exist in the new repo before running `git rm`.

---

### Task 1: Scaffold `~/projects/igbanam/orgchart`

**Files:**
- Create: `~/projects/igbanam/orgchart/` (copy of prototype minus `target/`, `.opencode/`)
- Create: `~/projects/igbanam/orgchart/LICENSE`

**Interfaces:**
- Produces: the new repo working tree that every later task edits; local path `$ORG` = `~/projects/igbanam/orgchart`.

- [ ] **Step 1: Copy the prototype, excluding `target/` and `.opencode/`**

```bash
mkdir -p ~/projects/igbanam/orgchart
rsync -a --exclude target/ --exclude .opencode/ \
  ~/projects/igbanam/ideas/prorotypes/ascii-org-chart/ \
  ~/projects/igbanam/orgchart/
```

This carries `src/`, `tests/`, `docs/` (including the release spec + this plan), `readme.md`, `Cargo.toml`, `Cargo.lock`, `.gitignore`.

- [ ] **Step 2: Create `LICENSE`** — standard MIT text; first line `Copyright (c) 2026 Owajigbanam Ogbuluijah <xigbanam@gmail.com>`. Copy the body verbatim from `~/projects/igbanam/reckless/LICENSE` and change the year to 2026.

- [ ] **Step 3: Verify nothing excluded came along**

Run: `ls -a ~/projects/igbanam/orgchart`
Expected: no `target`, no `.opencode`; `src tests docs readme.md Cargo.toml Cargo.lock .gitignore LICENSE` present.

- [ ] **Step 4: Verify the tests still pass in the new location**

Run: `cd ~/projects/igbanam/orgchart && cargo test --locked`
Expected: all 72 tests pass.

- [ ] **Step 5: Init git and commit**

```bash
cd ~/projects/igbanam/orgchart
git init -b main
git add -A
git commit -m "feat: import orgchart from igbanam/ideas (b9ca5c2)"
```

### Task 2: crates.io metadata + readme installation section

**Files:**
- Modify: `Cargo.toml` (add `[package]` metadata keys)
- Modify: `readme.md` (replace the "Installation" section)

**Interfaces:**
- Produces: package metadata crates.io requires; readme documents all 3 install channels.

- [ ] **Step 1: Add to `[package]` in `Cargo.toml`** (exact values):

```toml
description = "Render ASCII org charts from an indentation-based DSL"
license = "MIT"
repository = "https://github.com/igbanam/orgchart"
homepage = "https://github.com/igbanam/orgchart"
readme = "readme.md"
keywords = ["org-chart", "ascii", "cli", "diagram"]
categories = ["command-line-utilities"]
```

- [ ] **Step 2: Replace the readme "Installation" section** (currently `cargo install --path .`) with three subsections: `cargo install orgchart`, `brew install igbanam/servings/orgchart`, and "Download a binary" pointing at `https://github.com/igbanam/orgchart/releases` with the six-asset naming pattern (e.g. `orgchart-v0.1.0-aarch64-apple-darwin.tar.gz` for mac, `...-x86_64-unknown-linux-gnu.tar.gz` for linux, `...-x86_64-pc-windows-msvc.zip` for windows).

- [ ] **Step 3: Verify packaging is crates.io-valid**

Run: `cargo publish --dry-run` in `$ORG`
Expected: succeeds ("verify" completes, no missing-metadata errors). Also run `cargo package --list` and confirm no `target/` or `.opencode/` entries.

- [ ] **Step 4: Verify tests still pass**

Run: `cargo test --locked`
Expected: 72 pass.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml readme.md
git commit -m "feat: add crates.io metadata and installation docs"
```

### Task 3: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write `.github/workflows/ci.yml`** (exact copy):

```yaml
name: ci
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --locked
```

- [ ] **Step 2: Lint the YAML**

Run: `actionlint .github/workflows/ci.yml` (if actionlint is absent, `brew install actionlint` first)
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: test on linux, macos, windows"
```

(Push-time verification of the run itself happens in Task 5 alongside the release push.)

### Task 4: Release workflow + formula generator

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `scripts/homebrew-formula.sh`

**Interfaces:**
- Consumes: Task 1 repo, Task 2 version `0.1.0`.
- Produces: on tag push — GitHub release with 6 assets + `checksums.txt`; crates.io publish; commit to `igbanam/homebrew-servings` adding/updating `Formula/orgchart.rb`.
- `scripts/homebrew-formula.sh <version> <sha_x86_64_mac> <sha_arm64_mac> <sha_x86_64_linux> <sha_arm64_linux>` → formula text on stdout.

- [ ] **Step 1: Write `scripts/homebrew-formula.sh`** (exact copy; chmod +x):

```bash
#!/usr/bin/env bash
set -euo pipefail
VERSION="$1"; SHA_X64_MAC="$2"; SHA_ARM_MAC="$3"; SHA_X64_LINUX="$4"; SHA_ARM_LINUX="$5"
cat <<EOF
class Orgchart < Formula
  desc "Render ASCII org charts from an indentation-based DSL"
  homepage "https://github.com/igbanam/orgchart"
  version "$VERSION"
  license "MIT"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/igbanam/orgchart/releases/download/v$VERSION/orgchart-v$VERSION-x86_64-apple-darwin.tar.gz"
      sha256 "$SHA_X64_MAC"
    end
    if Hardware::CPU.arm?
      url "https://github.com/igbanam/orgchart/releases/download/v$VERSION/orgchart-v$VERSION-aarch64-apple-darwin.tar.gz"
      sha256 "$SHA_ARM_MAC"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/igbanam/orgchart/releases/download/v$VERSION/orgchart-v$VERSION-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "$SHA_X64_LINUX"
    end
    if Hardware::CPU.arm?
      url "https://github.com/igbanam/orgchart/releases/download/v$VERSION/orgchart-v$VERSION-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "$SHA_ARM_LINUX"
    end
  end

  def install
    bin.install "orgchart"
  end

  test do
    assert_match "orgchart", shell_output("#{bin}/orgchart --version")
  end
end
EOF
```

- [ ] **Step 2: Verify the script runs**

Run: `bash -n scripts/homebrew-formula.sh && scripts/homebrew-formula.sh 0.1.0 a b c d | head -5`
Expected: syntax OK; output starts `class Orgchart < Formula`.

- [ ] **Step 3: Write `.github/workflows/release.yml`** (exact copy):

```yaml
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --locked

  build:
    needs: test
    strategy:
      matrix:
        include:
          - { target: x86_64-unknown-linux-gnu, os: ubuntu-latest, cross: false }
          - { target: aarch64-unknown-linux-gnu, os: ubuntu-latest, cross: true }
          - { target: x86_64-apple-darwin, os: macos-latest, cross: false }
          - { target: aarch64-apple-darwin, os: macos-latest, cross: false }
          - { target: x86_64-pc-windows-msvc, os: windows-latest, cross: false }
          - { target: aarch64-pc-windows-msvc, os: windows-latest, cross: false }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - if: matrix.cross
        uses: taiki-e/install-action@v2
        with: { tool: cross }
      - if: ${{ !matrix.cross }}
        run: cargo build --release --locked --target ${{ matrix.target }}
      - if: matrix.cross
        run: cross build --release --locked --target ${{ matrix.target }}
      - name: Smoke test (native-target matrix entries only)
        if: matrix.target == 'x86_64-unknown-linux-gnu' || matrix.target == 'aarch64-apple-darwin' || matrix.target == 'x86_64-pc-windows-msvc'
        shell: bash
        run: |
          bin="target/${{ matrix.target }}/release/orgchart"
          [ "${{ matrix.os }}" = windows-latest ] && bin="$bin.exe"
          "$bin" --version
      - uses: actions/upload-artifact@v4
        with:
          name: orgchart-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/orgchart

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with: { path: dist }
      - name: Package
        run: |
          VERSION="${GITHUB_REF_NAME#v}"
          mkdir out
          for t in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
                   x86_64-apple-darwin aarch64-apple-darwin; do
            (cd "dist/orgchart-$t" && tar czf "../../out/orgchart-v$VERSION-$t.tar.gz" orgchart)
          done
          for t in x86_64-pc-windows-msvc aarch64-pc-windows-msvc; do
            (cd "dist/orgchart-$t" && zip "../../out/orgchart-v$VERSION-$t.zip" orgchart.exe)
          done
          (cd out && shasum -a 256 * > checksums.txt)
      - uses: softprops/action-gh-release@v2
        with:
          files: out/*
          generate_release_notes: true

  publish-crates:
    needs: release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Publish to crates.io
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
        run: |
          if [ -z "$CARGO_REGISTRY_TOKEN" ]; then
            echo "::warning::CARGO_REGISTRY_TOKEN not set — skipping crates.io publish"
            exit 0
          fi
          cargo publish --locked --token "$CARGO_REGISTRY_TOKEN"

  update-tap:
    needs: release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          repository: igbanam/homebrew-servings
          token: ${{ secrets.SERVINGS_TAP_TOKEN }}
          path: tap
      - uses: actions/checkout@v4
        with: { path: orgchart }
      - name: Update Formula/orgchart.rb
        env:
          SERVINGS_TAP_TOKEN: ${{ secrets.SERVINGS_TAP_TOKEN }}
        run: |
          if [ -z "$SERVINGS_TAP_TOKEN" ]; then
            echo "::warning::SERVINGS_TAP_TOKEN not set — skipping tap update (manual formula update needed)"
            exit 0
          fi
          VERSION="${GITHUB_REF_NAME#v}"
          dl() { curl -fsSL -o "$2" "https://github.com/igbanam/orgchart/releases/download/v$VERSION/$1"; }
          dl "orgchart-v$VERSION-x86_64-apple-darwin.tar.gz" amd64-mac.tar.gz
          dl "orgchart-v$VERSION-aarch64-apple-darwin.tar.gz" arm64-mac.tar.gz
          dl "orgchart-v$VERSION-x86_64-unknown-linux-gnu.tar.gz" amd64-linux.tar.gz
          dl "orgchart-v$VERSION-aarch64-unknown-linux-gnu.tar.gz" arm64-linux.tar.gz
          sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
          orgchart/scripts/homebrew-formula.sh "$VERSION" \
            "$(sha amd64-mac.tar.gz)" "$(sha arm64-mac.tar.gz)" \
            "$(sha amd64-linux.tar.gz)" "$(sha arm64-linux.tar.gz)" \
            > tap/Formula/orgchart.rb
          cd tap
          git config user.name "igbanam"
          git config user.email "xigbanam@gmail.com"
          git add Formula/orgchart.rb
          git commit -m "orgchart: v$VERSION" || echo "no formula changes"
          git push
```

- [ ] **Step 4: Lint the workflow**

Run: `actionlint .github/workflows/release.yml`
Expected: no errors. Also run the packaging loop's tar/zip lines locally against a scratch dir to confirm the `zip` binary exists (preinstalled on ubuntu runners; confirm with `which zip`, else `brew install zip` locally for the same test).

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml scripts/homebrew-formula.sh
git commit -m "ci: add 3-channel release workflow"
```

### Task 5: Publish repo, set secrets, cut v0.1.0

**Interfaces:**
- Consumes: Tasks 1–4 (repo ready to push).
- Produces: public `igbanam/orgchart` with a passing CI run and release `v0.1.0` (GitHub release always; crates.io + tap if secrets set).

- [ ] **Step 1: Create the GitHub repo and push**

```bash
cd ~/projects/igbanam/orgchart
gh repo create igbanam/orgchart --public --source . --push
gh run watch   # first ci.yml run on main must be green
```

- [ ] **Step 2: PARTNER BLOCKER — add two secrets** (I must stop and hand over here; tokens must be created by you):
  - `CARGO_REGISTRY_TOKEN`: token from https://crates.io/settings/tokens (scope: publish-new). Add via `gh secret set CARGO_REGISTRY_TOKEN -R igbanam/orgchart`.
  - `SERVINGS_TAP_TOKEN`: a PAT (fine-grained, limited to `igbanam/homebrew-servings`, contents read/write) created at https://github.com/settings/tokens. Add via `gh secret set SERVINGS_TAP_TOKEN -R igbanam/orgchart`.
  - If you skip either, the release still completes — that channel just logs a warning and defers to manual.

- [ ] **Step 3: Tag and push**

```bash
git tag v0.1.0
git push origin v0.1.0
gh run watch   # release.yml must go green
```

- [ ] **Step 4: Verify channel 1 (GitHub Releases)**

```bash
gh release view v0.1.0 --json assets --jq '.assets[].name'
```
Expected: 6 `orgchart-v0.1.0-<target>` archives + `checksums.txt`. Spot-check a non-native asset:

```bash
gh release download v0.1.0 --pattern 'orgchart-v0.1.0-aarch64-unknown-linux-gnu.tar.gz' -O /tmp/orgchart-arm.tar.gz
tar tzf /tmp/orgchart-arm.tar.gz   # lists exactly "orgchart"
shasum -a 256 /tmp/orgchart-arm.tar.gz   # matches checksums.txt
```

- [ ] **Step 5: Verify channel 2 (crates.io)**

If token was set: `cargo install orgchart --version 0.1.0` succeeds, and https://crates.io/crates/orgchart renders. If skipped: `cargo publish --dry-run` remains the standing gate; publish manually later with `cargo publish --token`.

- [ ] **Step 6: Verify channel 3 state (tap)**

If token was set: `Formula/orgchart.rb` exists in `igbanam/homebrew-servings` at version 0.1.0. If skipped: formula update is deferred to Task 6 manual step.

### Task 6: End-user verification, tap docs, ideas cleanup

**Files:**
- Create: `~/projects/igbanam/homebrew-servings/docs/orgchart.md`
- Modify: `~/projects/igbanam/homebrew-servings/README.md` (add orgchart line)
- Create/modify (in `igbanam/homebrew-servings` if Task 5 step 6 skipped): `Formula/orgchart.rb`
- Delete: `~/projects/igbanam/ideas/prorotypes/ascii-org-chart/` (git rm in ideas repo)

- [ ] **Step 1: Verify the formula end-to-end on this Mac (arm64)**

```bash
brew update
brew install igbanam/servings/orgchart
orgchart --version   # prints orgchart 0.1.0
echo 'A\n  B\n  C' | orgchart -   # renders a 3-node chart
```

If CI wrote the formula and anything fails, run `brew audit --formula igbanam/servings/orgchart` for diagnostics before editing anything.

- [ ] **Step 2: Add tap docs** — create `docs/orgchart.md` (one paragraph + `brew install igbanam/servings/orgchart` + pointer to the upstream repo), and add a README.md list entry following the mamiwota line's format:

```markdown
- [orgchart](./docs/orgchart.md): Render ASCII org charts from an indentation-based DSL
```

If CI didn't write the formula (Task 5 step 6 skipped): also add `Formula/orgchart.rb` generated by `scripts/homebrew-formula.sh` with shas from the release's `checksums.txt`.

- [ ] **Step 3: Commit and push the tap**

```bash
cd ~/projects/igbanam/homebrew-servings
git add Formula/orgchart.rb docs/orgchart.md README.md
git commit -m "orgchart: add formula and docs for v0.1.0"
git push
```

- [ ] **Step 4: Remove the prototype from `ideas`**

Guard first (Review Focus #5): `ls ~/projects/igbanam/orgchart/docs/superpowers/specs/` must show `2026-10-07-orgchart-release-design.md` — the docs have migrated. Then:

```bash
cd ~/projects/igbanam/ideas
git rm -r prorotypes/ascii-org-chart
git commit -m "orgchart: moved to igbanam/orgchart"
git push
```

- [ ] **Step 5: Final cross-channel sweep**

- `gh release view v0.1.0 -R igbanam/orgchart` — 6 binaries + checksums (channel 1)
- `cargo install orgchart` or crates.io page (channel 2)
- fresh shell: `brew install igbanam/servings/orgchart && orgchart --version` → `orgchart 0.1.0` (channel 3)
- `gh repo view igbanam/orgchart` — public, CI green
- `git -C ~/projects/igbanam/ideas log --oneline -2` — "moved to" commit is HEAD