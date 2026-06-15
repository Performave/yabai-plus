# Releasing yabai-plus

This document describes how a yabai-plus release is cut. Releases are produced
automatically by the [`release` workflow](../.github/workflows/release.yml) when a
version tag is pushed. You should rarely need to build a release by hand.

> One-time setup (Apple Developer account, GitHub secrets) lives in
> [ci-setup.md](./ci-setup.md). Read that first if releases have never run on
> this repo.

## TL;DR

```bash
# 1. Bump the version in src/yabai.c (MAJOR/MINOR/PATCH).
# 2. Add a CHANGELOG.md entry.
# 3. Commit, then tag and push:
git tag v7.1.25-plus.1
git push origin v7.1.25-plus.1
```

Pushing the tag triggers the workflow, which builds, signs, notarizes, and
creates a GitHub Release with the archive attached.

## Versioning scheme

yabai-plus tracks upstream yabai and adds patches on top. To stay distinguishable
from upstream while remaining sortable, releases use the upstream version plus a
`-plus.N` suffix:

```
v<upstream-version>-plus.<n>
   e.g. v7.1.25-plus.1, v7.1.25-plus.2, v7.1.26-plus.1
```

The version string is compiled into the binary from `src/yabai.c`:

```c
#define MAJOR 7
#define MINOR 1
#define PATCH 25
```

`yabai --version` prints `yabai-v7.1.25`, and the release archive is named from
that output (`yabai-v<version>.tar.gz`). If you want the `-plus.N` suffix to show
up in `--version` and the artifact name, extend the version macros / print format
in `src/yabai.c` accordingly. At minimum, bump `PATCH`/`MINOR`/`MAJOR` to match
the upstream base you rebased onto.

## What the release workflow does

On a `v*` tag push (`.github/workflows/release.yml`), a `macos-14` runner:

1. **Imports** the Developer ID Application certificate into a throwaway keychain.
2. **Builds** a universal (x86_64 + arm64) binary with `make install`.
3. **Builds** the man page (`make man`, needs `asciidoctor`).
4. **Codesigns** `bin/yabai` with the hardened runtime and a secure timestamp
   (`codesign --force --timestamp --options runtime --sign "$APPLE_SIGNING_IDENTITY"`).
5. **Notarizes** via `xcrun notarytool submit --wait` using an App Store Connect
   API key.
6. **Assembles** `bin/yabai-v<version>.tar.gz` containing `bin/`, `doc/`, `examples/`.
7. **Creates** a GitHub Release for the tag with the tarball attached.

## Signing & notarization notes

These are the non-obvious bits that cause most release failures:

- **Hardened runtime is required for notarization** (`--options runtime`). It does
  **not** interfere with the scripting addition — SA injection into Dock.app is
  gated by partially-disabled SIP + root, a separate mechanism. Keep the flag.
- **Only the main `bin/yabai` binary is signed.** The scripting-addition
  `payload`/`loader` are compiled into the binary (`src/osax/*_bin.c`) and injected
  into Dock at runtime; they must **not** be hardened-runtime signed. The workflow
  never touches them — leave it that way.
- **A bare CLI binary cannot be stapled.** Notarization still succeeds; Gatekeeper
  verifies the ticket online on first run. `xcrun stapler staple bin/yabai` will
  fail and that is expected. If you ever need offline-trusted installs, ship a
  `.pkg` and staple that instead.
- **Use a stable Developer ID across releases.** TCC (Accessibility/Automation)
  permissions are bound to the signature; keeping the same identity means users
  grant permission once and it persists across updates.

## Building a release manually (fallback)

If CI is unavailable:

```bash
make install          # universal build into bin/yabai
make man              # man page (requires asciidoctor)
codesign --force --timestamp --options runtime \
  --sign "Developer ID Application: <Your Name> (TEAMID)" bin/yabai
codesign --verify --strict --verbose=2 bin/yabai

# notarize
ditto -c -k --keepParent bin/yabai /tmp/yabai-notarize.zip
xcrun notarytool submit /tmp/yabai-notarize.zip \
  --key /path/to/AuthKey_XXXX.p8 --key-id <KEY_ID> --issuer <ISSUER_ID> --wait

# archive (matches install.sh expectations)
VERSION="$(bin/yabai --version)"
rm -rf archive && mkdir archive
cp -r bin doc examples archive/
tar -cvzf "bin/${VERSION}.tar.gz" archive
rm -rf archive
shasum -a 256 "bin/${VERSION}.tar.gz"

# publish
gh release create v7.1.25-plus.1 "bin/${VERSION}.tar.gz" --generate-notes
```

## After releasing

- If you maintain a Homebrew tap, update the formula/cask URL + sha256 to point at
  the new release tarball.
- `scripts/install.sh` carries a hard-coded `VERSION` + `EXPECTED_HASH` (upstream's
  curl installer). If you distribute via that script, run `make publish` to update
  those fields, or update them by hand.
