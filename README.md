# MatriXgram

A Telegram-like desktop client for the [Matrix](https://matrix.org/) protocol,
built with Tauri (Rust core + SvelteKit UI).

The goal is a messaging client that feels like Telegram Desktop: a flat chat
list, bubbles with tails, grouped messages, pinned chats, unread badges — while
speaking Matrix natively (federation, E2EE, Spaces, threads, MatrixRTC).

## Status

Early development. The Matrix protocol layer is mature (it comes from the
[Singularity](https://github.com/Luxuride/singularity) client); the Telegram
look and the chat-list features are work in progress.

See [ROADMAP.md](ROADMAP.md) for what is left, in priority order.

| Area | State |
|---|---|
| Login (password, SSO), session restore, account switching | working |
| E2EE (RustCrypto), SAS verification, recovery key | working |
| Room list, spaces, room settings, keywords | working |
| Message timeline, reply, reactions, media, emoji | working |
| Telegram bubbles, grouping, date separators | working |
| Last message preview, unread counts, pinned/muted rooms | in progress |
| Threads (MSC3440), edits, redactions, mentions | partially done |
| Sticker, voice message, gallery | not done |
| OS notifications, calls (MatrixRTC) | not done |

## Attribution and licence

This project is a fork of [Singularity](https://github.com/Luxuride/singularity)
by Luxuride, which is licensed under the **GPL-3.0-or-later**. The Rust core
(`src-tauri/libs/*`), the storage layer and the Tauri scaffolding come from
that project; the Telegram-style UI, the design tokens and the chat-list work
are new.

Because GPL-3.0 is copyleft, this fork stays GPL-3.0-or-later. The `LICENSE`
file is the GNU General Public License v3.

The Matrix Rust SDK (`matrix-rust-sdk`) and the JavaScript parts it uses are
Apache-2.0.

## Architecture

```
src-tauri/
├── libs/            Rust domain crates, Tauri-free and reusable
│   ├── types/       DTOs shared with the frontend (source of truth for
│   │                src/lib/generated/types.ts)
│   ├── protocol/    sync, event parsing, HTML sanitiser
│   ├── auth/        login flows, session persistence
│   ├── chat/        send/receive, media, reactions, emoji
│   ├── rooms/       room list, navigation, avatars
│   ├── verification/ device verification
│   ├── storage/     encrypted sqlite cache
│   ├── keychain/    OS credential store
│   ├── assets/      media cache
│   └── settings/    profile, keywords, room settings
└── src/             Tauri binder: commands and app setup only
src/                 SvelteKit UI
├── lib/components/
│   ├── messaging/   timeline, bubbles, composer
│   └── navigation/  chat list, room settings
└── lib/theme.ts     light/dark theme
```

The Rust crates never depend on Tauri: they take `&Paths`, `&Arc<AppDb>` and
`&Arc<AuthState>` and return plain data. The Tauri layer only adapts them to
IPC. That keeps the core portable to a mobile shell later.

## Prerequisites

### Option 1: VS Code Dev Containers (recommended)

1. Install the [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension in VS Code.
2. Open the workspace in a container:
   - Press `Ctrl+Shift+P` → **Dev Containers: Reopen in Container**
3. The container will build with all dependencies (Node.js, Rust, Tauri native libs) pre-installed.

### Option 2: Manual setup

Required tools:

- Node.js 22+
- npm
- Rust toolchain (stable)
- cargo-tauri
- On Windows: Visual Studio Build Tools with the "Desktop development with C++" workload

On Linux, you also need Tauri/WebKitGTK native dependencies.

## Getting Started

1. Install JS dependencies:

```bash
npm install
```

2. Run in desktop dev mode:

```bash
npm tauri dev
```

3. Build production artifacts:

```bash
npm tauri build
```

## Development Commands

Frontend checks (type generation + `svelte-check`):

```bash
npm run check
```

Frontend build:

```bash
npm run build
```

Rust checks and tests (from `src-tauri`):

```bash
cargo check --workspace --all-targets
cargo test --workspace
```

The TypeScript types in `src/lib/generated/types.ts` are generated from the
Rust `types` crate. After changing a struct in `src-tauri/libs/types/src/`,
run `npm run gen:types`; CI verifies the file is in sync.

## Theming

The app ships with the **Warp** theme (a Telegram Desktop theme by
[@ThemesM8](https://t.me/ThemesM8)) and can load any other Telegram Desktop
theme.

- **Settings → Appearance** holds the theme controls, in the spirit of
  Telegram Desktop: the base palette (light / dark / system), a list of
  Telegram themes, an importer for `.tdesktop-theme` files, and manual
  overrides for the accent colour, the background, the text colour and the
  font size.
- A `.tdesktop-theme` file is a ZIP archive with a `colors.tdesktop-theme`
  entry (Telegram's own colour names, `#rrggbbaa`) and an optional
  `background.jpg` wallpaper. The backend parses it and the frontend maps
  Telegram's colour names onto the app's CSS variables, so a theme written
  for the desktop client lands on the right parts of the UI.
- The palette, the overrides and the font size are stored in
  `localStorage`; the theme is applied before the first paint.

## Deep links

The app registers the `matrix://` scheme, so `matrix://room/!room:example.org`
and `matrix://join/#alias:example.org` open the right room. Note that Element
registers the same scheme on Windows: only one application can own it per
user session.
