# orgchart — Standalone Repository & 3-Channel Release

Date: 2026-10-07
Status: Approved design (this document); precedes implementation plan.

## Summary

Break `orgchart` (the ASCII org chart CLI currently living at
`ideas/prorotypes/ascii-org-chart` inside the `igbanam/ideas` repository)
out into its own public repository, `igbanam/orgchart`, and release it
on three channels — GitHub Releases, crates.io, and Homebrew — with
prebuilt binaries for Windows, Linux, and macOS (both x86_64 and
aarch64).

## Decisions (agreed with partner)

1. **Repo**: new public repo `igbanam/orgchart`; local checkout at
   `~/projects/igbanam/orgchart` (sibling of `ideas`, matching other
   project directories).
2. **Automation**: GitHub Actions release workflow, triggered by `v*`
   tag push. One command per release: tag + push.
3. **Homebrew**: new `Formula/orgchart.rb` added to the existing public
   tap `igbanam/homebrew-servings` (`brew install igbanam/servings/orgchart`).
4. **crates.io**: publish step in CI, gated on the
   `CARGO_REGISTRY_TOKEN` secret (partner adds the token once; exact
   steps handed over at that point in the plan).
5. **Original location**: remove `ideas/prorotypes/ascii-org-chart`
   from the `ideas` repo with a "moved to" commit.
6. **License**: MIT.
7. **Formula updates**: the release CI commits the updated formula to
   `homebrew-servings` automatically as part of a release.

## Non-goals

- Porting git history from `ideas` (the project is a single commit;
  fresh start is simpler and was implicitly accepted).
- Submitting to homebrew-core.
- Windows support in the Homebrew channel (Homebrew on Linux/macOS only).
- Any changes to `orgchart`'s features, CLI, or behavior.

## 1. Repository extraction

- Copy from the prototype: `src/`, `tests/`, `docs/`, `readme.md`,
  `Cargo.toml`, `Cargo.lock`, `.gitignore`.
- Do NOT copy: `target/` (build artifacts), `.opencode/` (session/goal
  state).
- New files:
  - `LICENSE` — MIT, copyright holder as in other repos of the author
    (verify against an existing repo, e.g. `reckless`).
  - `.github/workflows/ci.yml` — test workflow on push/PR (the 72
    existing tests, on all 3 OSes).
  - `.github/workflows/release.yml` — see section 2.
  - Expanded `readme.md` installation section covering all 3 channels.
- `Cargo.toml` gains crates.io-required metadata:
  - `license = "MIT"`
  - `description` (one line, from readme tagline)
  - `repository = "https://github.com/igbanam/orgchart"`
  - `homepage` (same), plus `categories`, `keywords`
- Fresh `git init`; initial commit(s) per conventional-commits.
- Push to new repo `igbanam/orgchart` (public), default branch `main`.

## 2. Channel 1 — GitHub Releases

`release.yml`, triggered on push of tags matching `v*`:

1. **Test job**: `cargo test` on ubuntu (guard before shipping).
2. **Build matrix** (6 targets):

   | Target                      | Runner         | Notes                        |
   |-----------------------------|----------------|------------------------------|
   | `x86_64-unknown-linux-gnu`  | ubuntu-latest  |                              |
   | `aarch64-unknown-linux-gnu` | ubuntu-latest  | cross-compile via `cross`    |
   | `x86_64-apple-darwin`       | macos-latest   | native build                 |
   | `aarch64-apple-darwin`      | macos-latest   | native build                 |
   | `x86_64-pc-windows-msvc`    | windows-latest | `.exe`, shipped as `.zip`    |
   | `aarch64-pc-windows-msvc`   | windows-latest | cross-compile, shipped `.zip` |

3. **Package**: each binary tarred (`.tar.gz` for unix, `.zip` for
3. **Package**: each binary tarred (`.tar.gz` for unix, `.zip` for
   windows) with SHA256 recorded; one `checksums.txt` per release.
4. **Publish job** (needs: tests + all builds):
   - `softprops/action-gh-release` creating the GitHub release with the
     6 archives + `checksums.txt`, tagged from the pushed tag.
   - `cargo publish` for crates.io using `secrets.CARGO_REGISTRY_TOKEN`
     (skip with warning if secret absent, so a token-less dry run
     still produces a GitHub release).
   - Formula update committed to `igbanam/homebrew-servings`
     (section 4).

Artifact naming: `orgchart-<version>-<target>.tar.gz` / `.zip`.

## 3. Channel 2 — crates.io

- `cargo publish --dry-run` validated locally before the first CI run.
- CI publishes with `CARGO_REGISTRY_TOKEN`; publishing occurs after the
  GitHub release is created (release artifacts are the primary
  deliverable; crates publish failure must not delete the release).
- Package name `orgchart` (verify availability on crates.io before
  first publish; if taken, stop and consult partner — do not pick a
  different name unilaterally).

## 4. Channel 3 — Homebrew

- New file `Formula/orgchart.rb` in `igbanam/homebrew-servings`.
- Formula pulls release binaries from `igbanam/orgchart` GitHub
  releases (NOT from the tap's own releases, unlike `reckless.rb`).
- `on_macos` / `on_linux` blocks with per-arch URLs where Homebrew's
  `Hardware::CPU` logic requires; SHA256 per platform from the release's
  `checksums.txt`.
- The release workflow generates and commits the formula with correct
  SHA256s automatically, on the tap's `main` branch, committed as the
  releasing CI identity using a `SERVINGS_TAP_TOKEN` secret (`repo`
  scope) added to `igbanam/orgchart` secrets.
- Homebrew convention: formula class name `Orgchart`.

## 5. Verification

Local, before tagging:
- `cargo test` green.
- `cargo publish --dry-run` passes.
- `cargo package --list` contents look right (no `target/`, no
  `.opencode/`).

After release (v0.1.0):
- GitHub release page shows 6 binary archives + `checksums.txt`.
- `cargo install orgchart` works (or crates.io page exists).
- `brew install igbanam/servings/orgchart` succeeds on partner's Mac
  (darwin arm64) and `orgchart --version` prints 0.1.0.
- Windows/Linux binaries spot-checked by downloading from the release
  (CI runs the binary smoke test as part of the build job: `orgchart
  --version` executes on each runner).

## Risks

- **crates.io name collision** (`orgchart` may be taken) — checked
  before first publish; if taken, partner consulted.
- **cross-compile of `notify`** — supports all 6 targets natively;
  `cross` used for Linux aarch64 as the standard workaround.
- **Windows aarch64 runner availability** — windows-latest runners are
  x86_64; aarch64 Windows binary built via cross-compile
  (`cargo build --target aarch64-pc-windows-msvc` works with the MSVC
  toolchain and a rustup target add).
- **Tap update via CI** requires a PAT with `repo` scope on
  homebrew-servings; without it, release still succeeds and formula
  update falls back to manual (warning logged).