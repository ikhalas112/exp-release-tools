# maxgame-release-tools

**Single-file** release engine (Rust). Windows: encrypts and compresses game assets into one protected `.exe` via [maxion-protector](https://github.com/maxion-game/maxion-protector). macOS: packs the `.app` bundle into one zip via `ditto` (no protection — maxion-protector is PE-only). Deploys to Cloudflare R2.

## Distribution model

**Prebuilt binary** — game CI downloads a Windows bundle from GitHub Releases. R2 and MAXION credentials are **embedded at tools build time** (not stored in game repos).

Bundle contents:

- `maxgame-release-windows-x86_64.zip` — `maxgame-release.exe` (release CLI) + `pnp.exe` (maxion-protector packer) + `maxion_stub.dll` (protector stub)
- `maxgame-release-macos-aarch64.tar.gz` — `maxgame-release` only (macOS lane uses `ditto`, no protector)

## Flow

```
git tag vX.Y.Z[-dev|sit|uat|staging]  →  Game CI (windows-latest)
                                            ├─ download prebuilt bundle
                                            ├─ resolve     tag → env/channel/R2 paths
                                            ├─ mock-build  fixture → build/game.exe + assets/
                                            ├─ inject      version.txt into assets/
                                            ├─ protect     maxion-protector → output/GameClient.exe
                                            ├─ manifest    manifest.json + channel-manifest.json
                                            ├─ sync        upload to releases/{tag}/
                                            ├─ update-channel  version-guarded pointer
                                            └─ verify      CDN check (.exe + channel manifest)
```

## Tag → channel mapping

| Tag | DEPLOY_ENV | Channel |
|-----|------------|---------|
| `vX.Y.Z` | prod | prod |
| `vX.Y.Z-dev` | dev | dev |
| `vX.Y.Z-sit` | sit | sit |
| `vX.Y.Z-uat` | uat | uat |
| `vX.Y.Z-staging` | staging | staging |

Build numbers supported: `v1.0.0-dev.2`

## Platforms

Default platform is `windows` — existing configs, R2 keys, and manifest JSON are unchanged. macOS is opt-in per game via a `macos` block in `release.config.json`:

```json
"macos": {
  "appBundle": "Game.app",
  "outputFile": "GameClient-macos.zip",
  "versionFileDir": "Contents/Resources",
  "mockFixture": "fixtures/mock-game-macos"
}
```

macOS pipeline: `resolve --platform=macos` → `mock-build --platform=macos` → `inject-version` (into the app bundle) → **`package`** (.app → single zip via `ditto -c -k`, preserves signatures) → `manifest --platform=macos` → `sync` → `update-channel --platform=macos` → `verify --platform=macos`.

R2 layout per platform:

| | Windows (unchanged) | macOS |
|---|---|---|
| releases | `releases/{tag}/…` | `releases/{tag}/macos/…` |
| channel manifest | `channels/{ch}/manifest.json` | `channels/{ch}/manifest-macos.json` |

The Windows sync step passes `--exclude-prefix=macos/` so its mirror delete never removes the macOS pipeline's nested artifacts. Code signing/notarization is out of scope (`ditto` preserves an existing signature). Windows and macOS are **separate reusable pipelines** — pick the one for your platform (a game targeting both calls both).

## Game CI integration (include-only)

No submodule. Pin a tools release tag that matches the prebuilt bundle.

**GitHub Actions** — Windows and macOS are split into two reusable pipelines:

- Windows: [`release-windows.reusable.yml`](.github/workflows/release-windows.reusable.yml) — protected `.exe` on `windows-latest`. Example: [`examples/github/release-windows.yml`](examples/github/release-windows.yml).
- macOS: [`release-macos.reusable.yml`](.github/workflows/release-macos.reusable.yml) — `.app` → zip via `ditto` on `macos-latest`. Example: [`examples/github/release-macos.yml`](examples/github/release-macos.yml).

```yaml
jobs:
  release:
    uses: maxion-game/maxgame-release-tools/.github/workflows/release-windows.reusable.yml@v0.3.0
    with:
      config: release.config.json
      tools_version: v0.3.0
    secrets:
      TOOLS_DOWNLOAD_TOKEN: ${{ secrets.TOOLS_DOWNLOAD_TOKEN }}  # required if tools repo is private
```

**GitLab** — include [`templates/gitlab/release-protector.yml`](templates/gitlab/release-protector.yml) and set `TOOLS_VERSION`.

Game repo needs only `release.config.json` (no R2 secrets).

## Tools repo: build & publish binary

Push tag `v*` on the tools repo → [`.github/workflows/release-binaries.yml`](.github/workflows/release-binaries.yml) builds and uploads the bundle.

**Secrets (tools repo only):**

- `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET_NAME`, `R2_ENDPOINT`
- `MAXION_BUILD_SECRET`

## Contributor local dev

`maxion-protector` is **vendored** in this repo (snapshot from upstream commit `6af572d`). Compile from source; credentials via `.env` (runtime env overrides embedded values):

```powershell
# Windows (for protect step)
cargo build --release -p maxion-stub --manifest-path maxion-protector/Cargo.toml
cargo build --release -p maxion-packer --manifest-path maxion-protector/Cargo.toml
cargo build --release
```

```powershell
maxgame-release resolve v1.0.0-dev --config=examples/release.config.json --format=env
maxgame-release mock-build --fixture=fixtures/mock-game --output=build
maxgame-release inject-version --source=build/assets --tag=v1.0.0-dev --channel=dev
maxgame-release protect --config=examples/release.config.json --build-dir=build --output-dir=output
maxgame-release manifest --config=examples/release.config.json --tag=v1.0.0-dev --artifact=output/GameClient.exe --output-dir=output
```

Local `.env` example:

```
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
R2_BUCKET_NAME=...
R2_ENDPOINT=...
MAXION_BUILD_SECRET=...
```

## Security notes

- Credentials in the release binary can be extracted — keep GitHub Release assets **private** and use `TOOLS_DOWNLOAD_TOKEN` for game CI downloads.
- Org-wide binary = shared R2 access; game paths are separated via `release.config.json` + tag resolve.
- Rotating credentials requires a new tools release tag and bumping `tools_version` in game workflows.

## Distribution model (launcher)

`distribution: single_exe` — launcher downloads the full protected executable (no `list.txt.gz` patch index).