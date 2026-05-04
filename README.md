# DirOtter

<p align="center">
  <img src="docs/assets/dirotter-icon.png" alt="DirOtter app icon" width="160">
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.de.md">Deutsch</a>
</p>

**DirOtter** is an open-source, local-first disk analyzer and cleanup assistant built with Rust.

It helps users understand where disk space is being used, identify large folders and files, review duplicate-file candidates, and safely clean low-risk cache or temporary files without uploading filesystem data to any cloud service.

DirOtter is designed to be transparent, privacy-preserving, and practical for everyday users who want a safer alternative to opaque disk-cleaning utilities.

## Project Status

DirOtter is currently in an early but production-ready stage.

The core Windows application is functional, tested, and packaged as a portable build. The project has passed the current quality gate across formatting, compilation, tests, linting, and workspace build validation.

Current validation status:

- `cargo fmt --all -- --check` passes
- `cargo check --workspace` passes with 0 errors and 0 warnings
- `cargo test --workspace` passes with 94 tests
- `cargo clippy --workspace --all-targets -- -D warnings` passes
- `cargo build --workspace` succeeds

The repository already includes CI workflows, Windows release packaging, portable installation scripts, and optional code-signing hooks.

## Why DirOtter Exists

Modern operating systems and applications generate large amounts of cache, temporary files, downloaded installers, duplicated assets, and hidden storage usage. Existing cleanup tools are often either too opaque, too aggressive, or too dependent on platform-specific assumptions.

DirOtter aims to provide a safer and more transparent approach:

1. Scan local disks using predictable strategies.
2. Explain what is using space.
3. Recommend cleanup candidates with risk levels.
4. Let users review before deleting.
5. Prefer reversible operations such as moving files to the recycle bin.
6. Keep filesystem data local by default.

The long-term goal is to provide a reliable open-source disk analysis and cleanup tool for Windows, macOS, and Linux.

## Core Features

### Disk Scanning

DirOtter scans selected directories and builds a structured view of disk usage.

The scanning pipeline supports:

- concurrent scanning
- batched publishing
- throttled UI updates
- cancellation
- completion-state handling
- lightweight session snapshots

The default user-facing scan mode focuses on a recommended strategy, while advanced scanning behavior can be adjusted for complex directories or large external drives.

### Cleanup Recommendations

DirOtter uses rule-based analysis to identify potential cleanup candidates.

Recommendation categories include:

- temporary files
- cache directories
- browser or app cache paths
- downloaded installers
- common low-risk generated files
- large files and folders that may deserve review

Recommendations are scored and grouped by risk level so that safer items are surfaced first.

### Duplicate File Review

DirOtter can identify duplicate-file candidates using a size-first strategy and background hashing.

The duplicate review flow is designed to avoid aggressive automatic deletion. It presents groups of candidates, recommends a file to keep, and avoids automatically selecting high-risk locations.

### Cleanup Execution

Supported cleanup actions include:

- move to recycle bin
- permanent deletion
- fast cleanup for low-risk cache candidates

Cleanup execution reports progress and result counts while processing files in the background.

### Local-First Storage

DirOtter does not require a database for normal usage.

Settings are stored in a lightweight `settings.json` file. Session results are stored only as temporary compressed snapshots and are removed when they are no longer needed.

If the settings directory is not writable, DirOtter falls back to temporary session storage and reports the fallback clearly in the settings UI.

### Internationalization

DirOtter supports language selection for 19 languages:

- Arabic
- Chinese
- Dutch
- English
- French
- German
- Hebrew
- Hindi
- Indonesian
- Italian
- Japanese
- Korean
- Polish
- Russian
- Spanish
- Thai
- Turkish
- Ukrainian
- Vietnamese

The current UI translation gate covers all supported languages for the shipped UI text. New user-visible UI strings should be translated for every selectable language before merging.

## Safety Model

DirOtter is intentionally conservative around deletion.

The project treats cleanup as a safety-sensitive operation because mistakes can result in data loss. As a result, DirOtter is designed around several safety principles:

- show cleanup candidates before execution
- classify recommendations by risk level
- prefer reversible deletion through the recycle bin
- avoid automatically selecting high-risk duplicate candidates
- keep permanent deletion explicit
- limit fast cleanup to low-risk cache or temporary paths
- surface operation results and failures clearly

Future work includes deeper safety auditing for platform-specific trash behavior, high-risk paths, symbolic links, permission failures, and irreversible deletion edge cases.

## Workspace Structure

```text
crates/
  dirotter-app        # Native application entry point
  dirotter-ui         # UI, pages, view models, interaction state
  dirotter-core       # Node store, aggregation, querying
  dirotter-scan       # Scanning event stream and aggregation publishing
  dirotter-dup        # Duplicate-file candidate detection
  dirotter-cache      # settings.json and session snapshot storage
  dirotter-platform   # Explorer integration, recycle bin, volumes, cleanup staging
  dirotter-actions    # Delete planning and cleanup execution
  dirotter-report     # Text, JSON, and CSV report export
  dirotter-telemetry  # Diagnostics and runtime metrics
  dirotter-testkit    # Regression and performance test utilities
```

## Build and Run

### Prerequisites

- Rust stable toolchain
- Cargo
- A supported desktop platform

Windows is currently the most mature target. macOS and Linux support are planned as part of the cross-platform roadmap.

### Run the Application

```bash
cargo run -p dirotter-app
```

### Release Build

```bash
cargo build --release -p dirotter-app
```

### Quality Gate

Before merging changes, the following checks should pass:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

## Release and Packaging

The repository includes a Windows release workflow and packaging scripts.

Current release-related components include:

- CI workflow for formatting, checking, testing, and linting
- Windows release workflow
- portable Windows packaging script
- optional Windows code-signing script
- portable installation script
- portable uninstallation script

Current Windows artifacts include a portable ZIP build and SHA-256 checksum file.

Code signing is supported by the release pipeline but requires secrets to be configured before producing signed builds.

## Roadmap

DirOtter is currently focused on improving reliability, safety, and cross-platform support.

High-priority and medium-priority roadmap items include:

1. Configure Windows code-signing secrets for signed release artifacts.
2. Add automated visual regression testing for the UI.
3. Expand Linux filesystem and trash/delete behavior coverage.
4. Expand macOS filesystem and trash/delete behavior coverage.
5. Audit cleanup and deletion safety boundaries.
6. Improve release automation and changelog generation.
7. Improve contributor documentation.
8. Add more integration tests for large directories, symlinks, permission errors, and external drives.
9. Keep all 19 UI languages covered as new user-visible strings are added.
10. Evaluate optional history persistence while keeping the default experience lightweight and local-first.

## How Codex Can Help This Project

DirOtter is a good fit for AI-assisted open-source maintenance because the project has a real multi-crate Rust codebase, safety-sensitive filesystem behavior, cross-platform goals, and ongoing maintainer workload.

Potential Codex-assisted open-source maintenance tasks include:

- reviewing Rust changes across the workspace
- triaging issues and reproducing bugs
- improving test coverage for scan, cleanup, duplicate detection, and reporting logic
- auditing cleanup safety rules
- checking platform-specific edge cases
- improving CI and release workflows
- generating and reviewing documentation updates
- helping maintain translation consistency
- drafting pull request summaries and release notes

Codex support would help keep the project fully open while reducing the maintenance burden required to make DirOtter safer, more reliable, and more useful across platforms.

## Contributing

Contributions are welcome.

Useful contribution areas include:

- filesystem scanning performance
- cleanup safety rules
- duplicate-file review UX
- Windows recycle-bin behavior
- Linux and macOS platform support
- UI testing
- visual regression testing
- accessibility improvements
- documentation
- translations
- packaging and release automation

Before submitting a pull request, please run the full quality gate:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

More detailed contributor documentation should be added in `CONTRIBUTING.md`.

## Security

DirOtter works with local filesystem data and cleanup operations, so security and safety are important project concerns.

Please report potential security or data-loss issues privately if possible. A dedicated `SECURITY.md` policy should define the preferred reporting channel, supported versions, and disclosure process.

Areas of special concern include:

- unsafe deletion behavior
- incorrect high-risk path classification
- symlink or junction traversal issues
- permission boundary issues
- platform-specific trash/recycle-bin failures
- irreversible deletion bugs
- incorrect cleanup recommendations

## Privacy

DirOtter is local-first.

The application is designed to analyze local filesystem metadata without uploading scan results, file paths, or cleanup recommendations to a cloud service by default.

Any future telemetry or crash reporting should be opt-in, clearly documented, and privacy-preserving.

## License

The workspace currently declares the project license as MIT in `Cargo.toml`. A root `LICENSE` file should be added before broader distribution.

## Project Goal

DirOtter aims to become a transparent, local-first, open-source disk analysis and cleanup tool that users can trust.

The project prioritizes:

- safety over aggressive cleanup
- explainability over opaque automation
- local processing over cloud dependency
- maintainability over short-term feature bloat
- cross-platform reliability over platform-specific shortcuts
