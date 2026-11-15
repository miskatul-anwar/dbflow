# Releasing DBFlow

This guide is for maintainers. Every release contains the desktop app (Windows, macOS, Linux) and the Android
app, built, checked and published together by `.github/workflows/release.yml`.

## Version scheme

We follow [Semantic Versioning](https://semver.org):

- **MAJOR**: breaking changes to outputs, workflow files or IPC contracts
- **MINOR**: new features (new operations, new output formats, new platforms)
- **PATCH**: bug fixes and docs

These carry the version and must agree. `bun run check:version` checks them; CI runs it on every pull request.

| File | Field |
|---|---|
| `package.json` | `version` |
| `src-tauri/tauri.conf.json` | `version` (the Android version name and code are derived from it) |
| `src-tauri/Cargo.toml` | `version` |
| `Cargo.toml` (workspace) | `workspace.package.version` (engine crates) |
| `Cargo.lock` | updated by `cargo update -w` |

## One-time setup

**Android signing secrets** (Settings → Secrets and variables → Actions). The release fails without them.
Every release must be signed with the same key, or Android refuses to install an update over the previous version.

| Secret | Value |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | the `.jks` keystore, base64-encoded (`base64 -w 0 dbflow-release.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | the keystore password |
| `ANDROID_KEY_ALIAS` | the key alias (e.g. `dbflow`) |
| `ANDROID_KEY_PASSWORD` | the key password |

Keep the keystore and its password backed up outside GitHub. Never commit them: `*.jks`, `*.keystore` and
`src-tauri/gen/android/keystore.properties` are ignored.

**GitHub Pages** (for the download page): Settings → Pages → Source: *GitHub Actions*.

## Release checklist

1. **Update `CHANGELOG.md`**: move the `Unreleased` entries under a new `## [X.Y.Z] - YYYY-MM-DD` heading
   ([Keep a Changelog](https://keepachangelog.com)). That section becomes the release notes.
2. **Bump the version** in the files above, then `cargo update -w` and `bun run check:version`.
3. **Merge to `main`** through a pull request, so CI has passed on it.
4. **Tag and push** from `main`:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
   Or run the *Release* workflow from the Actions tab on `main` with the tag `vX.Y.Z`; the tag is created on
   that commit when the release is published.
5. **The workflow does the rest**, in this order:
   - **prepare**: the tag must equal `v` + the version in every file, and the changelog must have the section.
     A *draft* release is created with that section as its notes.
   - **verify**: the full CI (frontend, Rust on Windows, macOS and Linux).
   - **desktop** and **android**, in parallel, upload to the draft:
     - Windows: `.exe` (NSIS) and `.msi`
     - macOS: `.dmg` for Apple Silicon and for Intel (and the `.app` archives)
     - Linux: `.AppImage`, `.deb` and `.rpm`
     - Android: `DBFlow_X.Y.Z_android_arm64.apk`, signed, its signature and version checked
   - **publish**: every expected file must be attached. `SHA256SUMS.txt` is added (from GitHub's digests),
     and the release is published and marked as latest.

   Nothing is public until every file is built, so the download page never shows a partial release.
6. **After the release**: check the [download page](https://miskatul-anwar.github.io/dbflow/) and install one build.

## When something fails

- **A build fails**: fix it, then *Re-run failed jobs* on the workflow run. The draft release is reused and files
  are replaced. If the fix needs new commits, delete the draft release and the tag, then tag the fixed commit.
- **"Release vX.Y.Z is already published"**: published releases are never overwritten. Bump the version.
- **"tag … does not match version …"**: the tag and the files disagree; fix the version or the tag.
- **Missing Android secret**: add it (see *One-time setup*) and re-run.
- **Linux webkitgtk errors**: the shared setup action (`.github/actions/setup`) installs
  `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`; keep that list current with Tauri.
- **macOS and Windows warnings on first launch**: the builds are not code-signed. The release notes explain the
  workaround.

## Hotfixes

Branch from the release tag, fix, bump the patch version, and release it like any other version:

```bash
git checkout -b hotfix/vX.Y.Z vX.Y.Z
# fix, bump to X.Y.(Z+1), update CHANGELOG.md, open a pull request to main, merge, tag
```

## Workflows

| Workflow | Runs on | Does |
|---|---|---|
| `ci.yml` | pull requests, pushes to `main`, called by the release | versions, typecheck, lint, tests, web build; Rust format, clippy and tests on Windows, macOS and Linux; an unsigned Android build |
| `release.yml` | `vX.Y.Z` tags, or manually with a tag | the release, as above |
| `pages.yml` | pushes to `main` that change `site/` | publishes the download page |

All three use `.github/actions/setup` for the toolchain. Dependabot keeps npm, Cargo, Gradle and the actions
up to date.
