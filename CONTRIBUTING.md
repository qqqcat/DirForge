# Contributing to DirOtter

DirOtter is a local-first disk analysis and cleanup tool. Contributions should keep filesystem safety, explainability, and cross-platform behavior ahead of convenience.

## Development Setup

1. Install the stable Rust toolchain.
2. Clone the repository.
3. Build and test from the repository root.

```bash
cargo check --workspace
cargo test --workspace
```

For Windows release packaging, use the repository script instead of assembling artifacts manually:

```powershell
./scripts/package-windows.ps1 -Configuration release
```

## Quality Gate

Run the full quality gate before opening a pull request:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

If your change affects release artifacts or documentation that describes packaged output, also run the relevant packaging script and verify the generated `BUILD-INFO.json`.

## Branch and Pull Request Process

- Use a focused branch for each change.
- Keep pull requests small enough to review directly.
- Describe the user-visible behavior change and the verification you ran.
- Link related issues when available.
- Avoid unrelated refactors, formatting churn, or cleanup in the same pull request.

## Issue Labels

Use labels to make maintenance work searchable:

- `security`: security, data-loss, permission-boundary, or cleanup-safety work.
- `cleanup-safety`: delete, recycle-bin, trash, path-risk, symlink, or junction behavior.
- `platform`: Windows, Linux, or macOS platform behavior.
- `ui`: user interface, accessibility, and visual regression work.
- `docs`: README, release notes, governance, and contributor documentation.
- `release`: packaging, changelog, signing, and distribution automation.
- `translation`: localization and language coverage.
- `testing`: unit, integration, regression, and visual test coverage.

## Safety-Sensitive Cleanup Changes

Cleanup and deletion behavior requires extra scrutiny. For changes touching delete, recycle-bin, trash, path classification, symlink, junction, permission, or irreversible action paths:

- Map the reader and writer path before editing stateful UI or runtime code.
- Identify the final writer that drives the effective UI state.
- Add or update a regression test for the exact failure mode.
- Document the manual verification path when automation is not practical.
- Prefer conservative behavior when platform APIs fail or return ambiguous results.

## Translation Rules

Any new user-visible UI text must be translated for every selectable language.

Do not add English fallback entries just to satisfy coverage tests. If a generated dictionary is missing a key, add a language-specific translation patch for every supported non-English language.

When fixing localization bugs, trace the final UI writer first and add a regression test that proves non-English languages do not render the raw English key.

## Release Checklist

Before publishing a release:

1. Confirm the workspace version in `Cargo.toml`.
2. Run the full quality gate.
3. Build the release application.
4. Run `./scripts/package-windows.ps1 -Configuration release` for Windows artifacts.
5. Verify `dist/DirOtter-windows-x64-{version}-portable/BUILD-INFO.json`.
6. Confirm whether the artifact is signed or explicitly documented as unsigned.
7. Update release notes and changelog material.
8. Confirm bundled documentation matches the source documentation.
