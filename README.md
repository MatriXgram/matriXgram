# MatriXgram

A Telegram-inspired desktop client for [Matrix](https://matrix.org/), built
with Tauri, Rust and SvelteKit.

## Features

- Matrix password and OAuth/SSO sign-in, saved sessions, rooms and spaces.
- Chat with replies, reactions, mentions, formatting, images, video and files.
- Unread markers, automatic history loading and jump to latest.
- Public-room explorer, room details, joining and user profiles.
- SAS device verification and customizable themes.
- Open message links in the system browser or copy them from the context menu.

Threads are partially implemented: thread relations are parsed, but thread
views and replies are not available yet. Sending edits and redactions, stickers,
voice messages, desktop notifications and calls are also not implemented.

## Getting started

Requirements: Node.js 22+, npm, stable Rust, and the native Tauri dependencies
for your operating system. On Windows, install Visual Studio Build Tools with
the Desktop development with C++ workload.

```bash
npm ci
npm run tauri -- dev
```

Build a release package with:

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

## Attribution and licence

MatriXgram is a fork of [Singularity](https://github.com/Luxuride/singularity)
by Luxuride, licensed under **GPL-3.0-or-later**. This project retains the
upstream GPL license. The Matrix Rust SDK and its JavaScript components are
Apache-2.0.

The Matrix name and logo are trademarks of The Matrix.org Foundation. This
project is independent and is not endorsed by or affiliated with the Foundation.
