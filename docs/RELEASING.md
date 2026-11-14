# Releasing DBFlow

This guide is for maintainers. The release process is fully automated once a
tag is pushed.

## Version scheme

We follow [Semantic Versioning](https://semver.org):

- **MAJOR** — breaking changes to outputs, workflow files, or IPC contracts
- **MINOR** — new features (new operations, new output formats…)
- **PATCH** — bug fixes and docs

Four files carry the version and must stay in sync:

| File | Field |
|---|---|
| `package.json` | `version` |
| `src-tauri/tauri.conf.json` | `version` |
| `src-tauri/Cargo.toml` | `version` |
| `Cargo.toml` (workspace) | `workspace.package.version` (engine crates) |

## Release checklist

1. **Freeze & verify**
   ```bash
   bun run typecheck
   bun run test --run
   cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
   bun run build
   ```
2. **Update `CHANGELOG.md`** — move the `Unreleased` section into a new
   `## [X.Y.Z] - YYYY-MM-DD` heading following
   [Keep a Changelog](https://keepachangelog.com).
3. **Bump the three versions** (same X.Y.Z everywhere).
4. **Commit**
   ```bash
   git add -A
   git commit -m "chore(release): v1.2.3"
   ```
5. **Tag & push**
   ```bash
   git tag vX.Y.Z
   git push origin main --tags
   ```
6. **CI does the rest** — `.github/workflows/release.yml` builds:
   - Windows: `.msi` + `.exe` (NSIS)
   - macOS: `.dmg` / `.app` for Apple Silicon *and* Intel
   - Linux: `.deb`, `.rpm`, `.AppImage`
   
   and publishes them on an automatically created GitHub Release named
   `DBFlow vX.Y.Z`.
7. **Post-release**
   - Verify assets on the [releases page](https://github.com/miskatul-anwar/dbflow/releases)
   - Announce in Discussions / socials as appropriate

## Hotfixes

Branch from the release tag:

```bash
git checkout -b hotfix/vX.Y.Z vX.Y.Z
# …fix, bump patch version, update CHANGELOG…
git tag vX.Y.Z+1 && git push origin main --tags
```

## Troubleshooting builds

- **macOS code-signing**: builds are unsigned by default; document the
  right-click → Open workaround in the release notes.
- **Linux webkitgtk failures**: the runner installs
  `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf` — keep
  this list current with Tauri's requirements.
- **Rust cache misses**: `Swatinem/rust-cache` keys off `Cargo.lock`; large
  dependency bumps will rebuild from scratch once.
