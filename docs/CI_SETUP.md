# CI setup

One-time setup required before the [`release` workflow](../.github/workflows/release.yml)
can build signed, notarized releases. Once these secrets exist, releasing is just
[pushing a tag](./RELEASING.md).

## Prerequisites

- An **Apple Developer Program** membership (needed for a "Developer ID
  Application" certificate and notarization).
- Admin access to the GitHub repo (to add Actions secrets).

## 1. Developer ID Application certificate

This is the certificate the binary is signed with.

1. In **Keychain Access** (or via Xcode → Settings → Accounts → Manage
   Certificates), create/download a **Developer ID Application** certificate.
2. Export it as a `.p12`: select the certificate **and** its private key →
   right-click → Export → `.p12`, and set an export password.
3. Note the exact identity name for later:
   ```bash
   security find-identity -v -p codesigning
   # -> "Developer ID Application: Your Name (TEAMID)"
   ```

## 2. App Store Connect API key (for notarization)

Using an API key avoids the 2FA/session flakiness of Apple-ID + app-specific
passwords.

1. Go to <https://appstoreconnect.apple.com> → **Users and Access** →
   **Integrations** → **App Store Connect API** → generate a key with the
   **Developer** role.
2. Record the **Key ID** and **Issuer ID** shown on that page.
3. Download the `AuthKey_XXXXXXXX.p8` file. **You can only download it once.**

## 3. GitHub Actions secrets

Add these under **Settings → Secrets and variables → Actions** on the repo. The
names intentionally match the `easel` repo, so the same values can be reused.

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | base64 of the `.p12`: `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the password set when exporting the `.p12` |
| `APPLE_KEYCHAIN_PASSWORD` | any random string (used for the temp CI keychain) |
| `APPLE_SIGNING_IDENTITY` | full identity, e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_API_KEY` | App Store Connect **Key ID** |
| `APPLE_API_ISSUER` | App Store Connect **Issuer ID** (a UUID) |
| `APPLE_API_PRIVATE_KEY` | raw contents of `AuthKey_XXXX.p8` (paste the PEM text, including the BEGIN/END lines) |

Scripting it instead of clicking:

```bash
gh secret set APPLE_CERTIFICATE          < <(base64 -i cert.p12)
gh secret set APPLE_CERTIFICATE_PASSWORD --body 'the-p12-password'
gh secret set APPLE_KEYCHAIN_PASSWORD    --body "$(openssl rand -hex 16)"
gh secret set APPLE_SIGNING_IDENTITY     --body 'Developer ID Application: Your Name (TEAMID)'
gh secret set APPLE_API_KEY              --body 'ABC123DEF4'
gh secret set APPLE_API_ISSUER           --body '00000000-0000-0000-0000-000000000000'
gh secret set APPLE_API_PRIVATE_KEY      < AuthKey_ABC123DEF4.p8
```

## 4. Why these specific choices

These mirror a setup that's known to work and sidestep common notarization
failures:

- **`echo -n ... | base64 --decode -o`** when importing the cert — the `-n` and
  `-o` avoid a trailing newline corrupting the `.p12`, a classic import failure.
- **App Store Connect API key** rather than Apple-ID auth — no 2FA prompts in CI.
- **`.p8` stored as raw text**, written with `printf` — avoids a base64
  round-trip getting mangled.
- **`set-key-partition-list`** after import — on recent macOS runners, `codesign`
  can fail non-interactively without it even when the key was imported with `-A`.

## 5. Verify

1. Confirm all seven secrets exist (`gh secret list`).
2. Trigger a run: either push a `v*` tag, or use the **workflow_dispatch** button
   on the Actions tab.
3. Watch the **Notarize** step — `notarytool ... --wait` blocks until Apple
   returns `Accepted` (or a rejection with a log URL).

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| `security import` fails | bad `APPLE_CERTIFICATE` base64 (re-export, use `base64 -i`) or wrong `.p12` password |
| `codesign` errors with "no identity found" | `APPLE_SIGNING_IDENTITY` doesn't match `security find-identity` output exactly |
| `codesign` hangs / "User interaction is not allowed" | missing `set-key-partition-list` step |
| notarytool `Invalid` | binary not signed with `--options runtime` / `--timestamp`, or signed with a non-Developer-ID cert |
| notarytool auth error | wrong Key ID / Issuer ID, or the `.p8` text is incomplete |
| Release not created | tag didn't match `v*`, or `contents: write` permission missing |
