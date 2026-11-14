# MatriXgram

<img width="590" height="128" alt="MatriXgram" src="https://github.com/user-attachments/assets/319e05b6-7e59-4b30-911d-1f87ec128475" />

A Telegram-inspired desktop client for [Matrix](https://matrix.org/), built with
Tauri, Rust and SvelteKit.

## Features

- Password and OAuth/SSO sign-in, saved sessions, rooms and spaces.
- Messages with replies, reactions, mentions, edits, formatting, images, video
  and files; image viewer with zoom and navigation.
- Unread markers, automatic history loading, pinned messages and jump to latest.
- Public-room explorer, room settings and details, member list, user profiles,
  and join/leave notices.
- SAS device verification, cache controls and customizable themes.
- Link previews and actions to open links in the system browser or copy them.

Thread relations are parsed, but thread views and replies are not yet available.
Stickers, voice messages, desktop notifications and calls are not implemented.

## Getting started

Requirements: Node.js 22+, npm, stable Rust, and the native Tauri dependencies
for your operating system. On Windows, install Visual Studio Build Tools with
the Desktop development with C++ workload.

```bash
npm ci
npm run tauri -- dev
```

Build a local release package with:

```bash
npm run tauri -- build
```

## Development checks

```bash
npm run check
cd src-tauri
cargo check --workspace --all-targets
cargo test --workspace
```

## Releases

Push a version tag (for example, `v0.1.1`) to run the release workflow. It builds
Windows installers and Linux DEB, RPM and Flatpak packages, then publishes them
with generated release notes. Installer assets are removed from older releases;
their release pages, notes and source archives are kept.

## Attribution and licence

MatriXgram is a fork of [Singularity](https://github.com/Luxuride/singularity)
by Luxuride, licensed under **GPL-3.0-or-later**. This project retains the
upstream GPL license. The Matrix Rust SDK and its JavaScript components are
Apache-2.0.

The Matrix name and logo are trademarks of The Matrix.org Foundation. This
project is independent and is not endorsed by or affiliated with the Foundation.
