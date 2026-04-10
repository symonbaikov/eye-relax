<div align="center">

<img src="src-tauri/icons/logo.png" alt="Blinkly logo" width="128" height="128" />

# Blinkly

**Gentle reminders to rest your eyes — built for Linux.**

Desktop companion that nudges you away from the screen with soft overlays, following the 20-20-20 rule. Runs quietly in the tray, respects your focus, and never gets in the way.

[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/react-19-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

</div>

---

## ✨ Features

- 👁️ **20-20-20 rule** — short breaks every 20 minutes, long breaks every hour
- 🪟 **Soft overlay** — translucent break window with a circular countdown, not a screen takeover
- 💤 **Smart idle detection** — timer auto-resets when you step away, no wasted breaks
- 🔕 **Snooze & Skip** — one click to postpone or skip when you're in the zone
- 📊 **Activity stats** — track work time, completed breaks, and skip rate over 7 days
- 🌗 **Light & dark themes** — follows your system appearance
- 🔔 **Native notifications** — D-Bus integration with action buttons as a fallback
- 🎨 **Tray-first UX** — no main window, just a quiet icon in your system tray
- 🦀 **Tiny footprint** — under 50 MB RAM, 0% CPU when idle

---

## 🖥️ Supported Systems

Blinkly is a **Linux-first** application, built and tested on modern desktop environments.

### Operating systems

| Distribution             | Status         |
| ------------------------ | -------------- |
| Ubuntu 22.04 / 24.04 LTS | ✅ Supported   |
| Fedora 39+               | ✅ Supported   |
| Arch Linux (rolling)     | ✅ Supported   |
| Debian 12+               | ✅ Supported   |
| openSUSE Tumbleweed      | ✅ Should work |

### Desktop environments

| DE                         | Wayland           | X11            |
| -------------------------- | ----------------- | -------------- |
| GNOME 45+                  | ✅ Primary target | ✅ Supported   |
| KDE Plasma 6+              | ✅ Supported      | ✅ Supported   |
| Other DEs with system tray | ⚠️ May work       | ✅ Should work |

### Display servers

- **Wayland** — primary target, uses `ext_idle_notify_v1` for idle detection
- **X11** — full fallback support via `XScreenSaverQueryInfo`

### System requirements

| Component | Minimum                                            |
| --------- | -------------------------------------------------- |
| Kernel    | Linux 5.15+                                        |
| glibc     | 2.31+                                              |
| RAM       | 128 MB free                                        |
| Disk      | 50 MB                                              |
| Runtime   | `libwebkit2gtk-4.1`, `libgtk-3`, D-Bus session bus |

> **macOS and Windows** are not supported in v1.0 — they are planned for v2.0.

---

## 📦 Installation

Download all Linux builds from the [GitHub Releases page](https://github.com/symonbaikov/eye-relax/releases).

| Your system                  | What to download | Notes                  |
| ---------------------------- | ---------------- | ---------------------- |
| Ubuntu / Debian              | `.deb`           | Native package install |
| Fedora                       | `.rpm`           | Native package install |
| Not sure / want auto-updates | `AppImage`       | Recommended fallback   |

### AppImage (recommended)

AppImage is the easiest portable build and the only Linux format that supports Blinkly's in-place auto-updates.

```bash
chmod +x blinkly_0.1.0_amd64.AppImage
./blinkly_0.1.0_amd64.AppImage
```

If FUSE is unavailable on your system, use the extraction fallback:

```bash
./blinkly_0.1.0_amd64.AppImage --appimage-extract-and-run
```

### Ubuntu / Debian

```bash
sudo dpkg -i blinkly_0.1.0_amd64.deb
sudo apt-get install -f
```

### Fedora

```bash
sudo dnf install ./blinkly-0.1.0-1.x86_64.rpm
```

### Updates

- `AppImage` users get automatic in-place updates from within Blinkly.
- `.deb` and `.rpm` users get a native notification plus a download prompt in Settings that opens the latest release page.

---

## 🚀 Usage

1. Launch Blinkly — it lives in your system tray, not as a window.
2. Right-click the tray icon for the menu: pause, break now, settings, stats, quit.
3. Every 20 minutes, a soft overlay appears with a 20-second countdown.
4. Click **Skip** or **Snooze** if you need to — Blinkly never forces anything.
5. Configure intervals, sound, theme, and autostart from the **Settings** panel.

The tray icon changes color to reflect state:

- 🟢 **Green eye** — working, timer is ticking
- 🟡 **Yellow eye** — on break
- ⚪ **Gray eye** — paused

---

## 🛠️ Development

Blinkly is built with **Tauri 2**, **Rust**, **React 19**, **TypeScript**, and **Tailwind CSS**.

### Prerequisites

- Rust (stable) — [rustup.rs](https://rustup.rs/)
- Node.js 20+ and npm
- Linux build dependencies: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`

### Run in dev mode

```bash
npm install
npm run tauri dev
```

### Build a release

```bash
npm ci
npm run tauri build -- --bundles appimage,deb,rpm
```

Artifacts end up in `src-tauri/target/release/bundle/`.

If you want updater signatures locally, export your signing key first:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/blinkly.key")"
npm run tauri build -- --bundles appimage,deb,rpm
```

On newer Fedora-like systems, AppImage bundling may also need `NO_STRIP=1` because the cached `linuxdeploy` tool can choke on modern `.relr.dyn` sections:

```bash
NO_STRIP=1 npm run tauri build -- --bundles appimage,deb,rpm
```

### Quality gates

```bash
cargo clippy -- -D warnings
npm run build
npm run lint
npm run typecheck
cargo test --all-targets
```

---

## 🏗️ Architecture

Blinkly follows a three-layer architecture with strict separation of concerns:

```
┌──────────────────────────────────┐
│      Rust Backend (Tauri 2)      │  Scheduler FSM · Activity Tracker
│                                   │  Notifications · Config · Stats
└──────────────┬───────────────────┘
               │ Tauri IPC
┌──────────────▼───────────────────┐
│   React + TypeScript + Zustand   │  Overlay · Settings · Stats
└──────────────┬───────────────────┘
               │
┌──────────────▼───────────────────┐
│             SQLite                │  config · breaks · sessions
└──────────────────────────────────┘
```

See [`.claude/rules/architecture.md`](.claude/rules/architecture.md) for the full design document.

---

## 📄 License

MIT — see [LICENSE](LICENSE) for details.

---

<div align="center">

Made with 🦀 and care for tired eyes.

</div>
