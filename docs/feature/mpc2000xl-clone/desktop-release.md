# Desktop Release Operations

The desktop app is packaged by `.github/workflows/desktop-release.yml`.

## Manual downloadable builds

Use GitHub Actions → `Desktop Release` → `Run workflow` on the branch you want to package. The workflow uploads three downloadable artifacts:

- `mpc2000xl-clone-desktop-linux-x86_64.tar.gz`
- `mpc2000xl-clone-desktop-macos-arm64.tar.gz`
- `mpc2000xl-clone-desktop-windows-x86_64.zip`

## Versioned releases

Push a version tag to publish GitHub Release assets:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow builds the desktop binary on Linux, macOS, and Windows, packages each build, and creates or updates the matching GitHub Release with those assets.

## Asset boundary

Release archives include the desktop executable and README only. They do not include user WAVs, `.mpc2000xl-project.json` project files, firmware images, proprietary Akai assets, or local sample folders.
