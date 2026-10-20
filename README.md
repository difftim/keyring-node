# `@napi-rs/keyring`

![https://github.com/Brooooooklyn/keyring-node/actions](https://github.com/Brooooooklyn/keyring-node/workflows/CI/badge.svg)
[![Downloads](https://img.shields.io/npm/dm/@napi-rs/keyring.svg?sanitize=true)](https://npmcharts.com/compare/@napi-rs/keyring?minimal=true)

> https://github.com/hwchen/keyring-rs Node.js binding via https://napi.rs

# Usage

```js
import { Entry } from '@napi-rs/keyring'

const entry = new Entry('my_service', 'my_name')
entry.setPassword('topS3cr3tP4$$w0rd')
const password = entry.getPassword()
console.log('My password is ', password)
entry.deletePassword()
```

# node-keytar compatibility

Two modules expose node-keytar's API (`getPassword`, `setPassword`, `deletePassword`,
`findPassword`, `findCredentials`); they differ only in where the credentials live:

| Module                           | Storage format                                        | Use when                                                                                |
| -------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `@napi-rs/keyring/keytar`        | this library's `Entry` format (unchanged since 2.1.x) | you only want keytar's API shape, or you already store credentials through this package |
| `@napi-rs/keyring/keytar-compat` | node-keytar's exact on-store format                   | you are migrating an app off node-keytar and its users already have stored credentials  |

## `keytar-compat`: zero-migration drop-in for node-keytar

`@napi-rs/keyring/keytar-compat` reads and writes credentials in keytar's exact on-store
format on every platform, so switching an existing application requires **no data migration in
either direction** — credentials written by keytar stay readable here, and credentials written
here stay readable by keytar:

```diff
- const keytar = require('keytar')
+ const keytar = require('@napi-rs/keyring/keytar-compat')
```

Per-platform storage identity (matching keytar exactly):

- **macOS**: generic passwords keyed by service/account in the login keychain (identical to
  the main `Entry` API, so the two can be mixed freely there).
- **Windows**: generic credentials with target name `{service}/{account}` and UTF-8 password
  blobs (the main `Entry` API uses `{user}.{service}` targets with UTF-16 blobs, which is why
  it cannot see keytar credentials).
- **Linux/FreeBSD**: Secret Service items with `{service, account}` attributes and the generic
  libsecret schema (the main `Entry` API uses `{service, username}`), matched on `xdg:schema`
  during lookups exactly as libsecret did for keytar.

Three known, deliberate divergences from keytar, all strictly narrower in scope: on Windows,
credentials are enumerated with the filter `{service}/*` where keytar's `{service}*` also matched
unrelated services sharing the prefix; on macOS, lookups are pinned to the login keychain rather
than the whole default keychain search list; and `findPassword` returns the first credential found
by `findCredentials`, so a Linux item that carries no `account` attribute (never written by keytar
itself) is not surfaced.

Note that `keytar-compat` and the main `Entry`/`AsyncEntry` API only share storage on macOS;
on Windows and Linux they address different credentials by design. The `keytar` shim shares
storage with `Entry` on every platform.

# Linux backend selection

On Linux, `Entry` and `AsyncEntry` pick a credential store automatically: the
[Secret Service](https://crates.io/crates/dbus-secret-service-keyring-store) (gnome-keyring, KWallet,
keepassxc, ...) is tried first, and the binding silently falls back to the
[kernel keyutils keyring](https://crates.io/crates/linux-keyutils-keyring-store) when no Secret Service
is available.

You can pin an entry to one specific store by passing the options bag as the last argument (the
option is accepted on every platform but only meaningful on Linux):

```js
new Entry('my_service', 'my_name', { linux: { store: 'secret-service' } }) // require Secret Service
new Entry('my_service', 'my_name', { linux: { store: 'keyutils' } }) // require the kernel keyring
Entry.withTarget('target', 'my_service', 'my_name', { linux: { store: 'keyutils' } })
```

When a store is required, the constructor throws if that store is unavailable — there is no silent
fallback. When `withTarget` is combined with the keyutils store, the target is used as the kernel key
description, so distinct targets keep distinct credentials.

> **Note:** the keyutils store keeps credentials in kernel memory: per the
> [linux-keyutils-keyring-store](https://crates.io/crates/linux-keyutils-keyring-store) docs, the key
> management facility "is completely in-memory and will not persist across reboots". Prefer
> `secret-service` for credentials that must survive a restart.

# macOS backend selection

On macOS, `Entry` and `AsyncEntry` use the legacy file-based
[keychain](https://crates.io/crates/apple-native-keyring-store) by default. You can pin an entry to a
specific store the same way as on Linux (the option is accepted on every platform but only meaningful
on macOS):

```js
new Entry('my_service', 'my_name', { macos: { store: 'keychain' } }) // the file-based keychain (default)
new Entry('my_service', 'my_name', { macos: { store: 'protected' } }) // the data-protection keychain
```

The `protected` store writes items to the data-protection keychain
(`kSecUseDataProtectionKeychain`), the modern store whose items are protected by the device
passcode / Secure Enclave policies.

- The calling process must be code-signed with an application identifier (an app bundle with a
  provisioning profile, or a signed binary with the right entitlements). Unsigned processes — plain
  `node` included — get "A required entitlement isn't present" from the OS on every operation.
- `withTarget` is not supported with the `protected` store and throws: on macOS the target selects a
  legacy keychain file, which has no meaning in the data-protection keychain.
- `service` and `username` must be non-empty: an empty string acts as a wildcard during lookup.
- `findCredentials` searches the legacy keychains only; it does not see items stored in the
  data-protection keychain.

With the `protected` store you can also choose the accessibility (`kSecAttrAccessible`) of the
credentials an entry creates. The system default, `when-unlocked`, makes an item readable only while
the device is unlocked; `after-first-unlock` keeps it readable from the first unlock after boot
until restart, which is the usual choice for background processes that need credentials while the
screen is locked:

```js
new Entry('my_service', 'my_name', { macos: { store: 'protected', accessPolicy: 'after-first-unlock' } })
```

The accepted values, from least to most restrictive: `after-first-unlock`,
`after-first-unlock-this-device-only`, `when-unlocked` (the default), `when-unlocked-this-device-only`,
`when-passcode-set-this-device-only`, and `require-user-presence`. The first five map to the
`kSecAttrAccessible` constants of the same name; `require-user-presence` is instead an access-control
policy — like `when-unlocked`, but every access additionally prompts for biometry or the device
password, so avoid it for background or headless processes where reads would block or fail. The
`-this-device-only` variants additionally prevent the item from being migrated to another device (it
still appears in encrypted backups, but cannot be restored onto different hardware). The policy
applies to credentials at creation time — it does not rewrite items that already exist. The file-based
`keychain` store has no per-item accessibility, so combining `accessPolicy` with it (explicitly or by
leaving `store` absent) throws rather than silently ignoring the option.

# Building and publishing (GitHub Packages)

This fork is published as `@difftim/keyring` to GitHub Packages, built and released locally from a
Mac. Only macOS binaries (arm64 + x64, as separate platform packages) are shipped for now; npm picks
the right one at install time via the `os`/`cpu` fields on the `optionalDependencies`.

## One-time setup

1. Install a rustup-managed toolchain with both macOS targets. Homebrew's `rust` formula only ships
   the host's standard library and cannot cross-compile to x64:

   ```sh
   brew install rustup   # keg-only: does not shadow an existing Homebrew rust
   export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
   rustup default stable
   rustup target add aarch64-apple-darwin x86_64-apple-darwin
   ```

2. Create a GitHub **classic** personal access token with the `write:packages` scope and add it to
   `~/.npmrc`:

   ```
   //npm.pkg.github.com/:_authToken=<PAT>
   ```

## Release

```sh
# 1. bump the version and commit
npm version patch

# 2. build both architectures (must use the rustup toolchain, not Homebrew rust)
PATH="/opt/homebrew/opt/rustup/bin:$PATH" yarn build:darwin

# 3. generate the per-platform package dirs and stage the binaries
yarn napi create-npm-dirs
cp keyring.darwin-arm64.node npm/darwin-arm64/
cp keyring.darwin-x64.node npm/darwin-x64/

# 4. publish — prepublishOnly publishes the two platform packages first,
#    injects them as optionalDependencies, then publishes the main package
npm publish

# 5. discard the optionalDependencies prepublish injected into package.json.
#    They belong only in the published tarball: committing them breaks
#    `yarn install` in this repo, which would try to resolve the platform
#    packages from the registry.
git checkout -- package.json
```

## Installing from GitHub Packages

GitHub Packages requires authentication even for public packages. Consumers need an `.npmrc` with:

```
@difftim:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=<PAT with read:packages>
```

Then `npm install @difftim/keyring` works on both Apple Silicon and Intel Macs.

## Adding Linux / Windows later

Add the targets back to `napi.targets` in `package.json` (e.g. `x86_64-unknown-linux-gnu`,
`x86_64-pc-windows-msvc`), build those binaries (CI or cross-compilation), stage them into
`npm/<platform>/`, and publish a new version. `napi prepublish` refuses to publish while any
declared target's binary is missing, so only declare targets you can actually ship.

# Bundled Rust dependencies

The prebuilt `.node` binaries are built with `--locked` from the committed [`Cargo.lock`](./Cargo.lock), and with
[`cargo auditable`](https://github.com/rust-secure-code/cargo-auditable), which embeds the exact list of Rust crates
compiled into each binary. Recover it for license compliance or vulnerability scanning with:

```sh
cargo install rust-audit-info
rust-audit-info node_modules/@napi-rs/keyring-darwin-arm64/keyring.darwin-arm64.node

# or scan for known vulnerabilities
cargo audit bin node_modules/@napi-rs/keyring-darwin-arm64/keyring.darwin-arm64.node
```

[syft](https://github.com/anchore/syft), [trivy](https://github.com/aquasecurity/trivy) and
[grype](https://github.com/anchore/grype) read the same data.

Each platform package also ships a `LICENSE.third-party.md` with the license texts of those crates, generated by
[`cargo-about`](https://github.com/EmbarkStudios/cargo-about). After changing `Cargo.lock`, regenerate them with
`yarn licenses` (needs `cargo install --locked cargo-about --features cli`); CI fails while they are stale.

> **Note:** the Linux binaries statically link `libdbus` (built from the sources vendored by
> [`libdbus-sys`](https://crates.io/crates/libdbus-sys)), which is licensed under `AFL-2.1 OR GPL-2.0-or-later`
> rather than the crate's own `MIT OR Apache-2.0`. The Linux `LICENSE.third-party.md` files include its license and
> copyright notices.
