# MatriXgram

<img width="590" height="128" alt="MatriXgram" src="https://github.com/user-attachments/assets/319e05b6-7e59-4b30-911d-1f87ec128475" />

A desktop client for [Matrix](https://matrix.org/) with a Telegram-inspired
interface, built with Tauri, Rust and SvelteKit.

## Features

- Password and OAuth/SSO sign-in, direct chats, rooms and spaces.
- Replies, reactions, mentions, edits, formatted messages, images, video and
  files, with media preview and image zoom.
- Unread markers, history loading, pinned messages and jump-to-message.
- Public-room directory, room and member details, user profiles, and room
  membership notices.
- Device verification, cache controls, customizable themes, and link actions.

Thread relations are parsed, but thread views and replies are not available yet.
Stickers, voice messages and calls are not implemented.

## Getting started

Requirements: Node.js 22+, npm, stable Rust, and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)
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

## Checks

```bash
npm run check
cd src-tauri && cargo fmt --all -- --check
cd src-tauri && cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Releases

Push a version tag (for example, `v0.1.1`) to run the release workflow. It runs
the checks above, builds Windows installers and Linux DEB, RPM and Flatpak
packages, and publishes a GitHub Release with generated release notes. Installer
assets are removed from older releases; their release pages, notes and source
archives remain available.

## Attribution and licence

MatriXgram is a fork of [Singularity](https://github.com/Luxuride/singularity)
by Luxuride and is licensed under **GNU GPL v3 or later**. See [LICENSE](LICENSE)
for the full terms. Third-party components retain their respective licenses.

The Matrix name and logo are trademarks of The Matrix.org Foundation. This
project is independent and is not endorsed by or affiliated with the Foundation.
