<p align="center">
  <img src="assets/yumic.svg" alt="YuMic Logo" width="128" height="128"/>
</p>

<h1 align="center">YuMic</h1>

<p align="center">
  <strong>Native Linux client for WO Mic (Android & iOS) with a gorgeous GTK4 + Libadwaita interface.</strong>
</p>

<p align="center">
  <a href="https://github.com/Excez0/yumic-server/releases">
    <img src="https://img.shields.io/github/v/release/Excez0/yumic-server?style=flat-square&label=Latest" alt="Release"/>
  </a>
  <a href="https://aur.archlinux.org/packages/yumic-server">
    <img src="https://img.shields.io/aur/version/yumic-server?style=flat-square&label=AUR" alt="AUR"/>
  </a>
  <a href="https://github.com/Excez0/yumic-server/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/Excez0/yumic-server?style=flat-square" alt="License"/>
  </a>
  <img src="https://img.shields.io/badge/platform-Linux-blue?style=flat-square" alt="Platform"/>
  <img src="https://img.shields.io/badge/language-Rust-orange?style=flat-square" alt="Rust"/>
</p>

---

## Why YuMic?

**WO Mic** is a great app that turns your phone into a wireless microphone, but its Linux support is practically non-existent. The official CLI client is closed-source, hard to set up, requires obsolete ALSA loopback kernel modules, and offers no visual feedback.

**YuMic** is a modern, open-source alternative written in Rust. It provides a clean GTK4 UI with live audio metering, automatic PipeWire virtual microphone routing, system tray integration, and auto-reconnect.

---

## Features

- **Real-time Opus Decoding** — 48kHz mono Opus stream decoded with zero errors
- **GTK4 + Libadwaita UI** — Native look on GNOME, KDE, and other modern desktops
- **Custom Cairo Audio Meter** — Gradient-colored level bar with peak hold
- **System Tray** — Minimize to tray via pure DBus SNI (`ksni`), right-click menu for quick actions
- **PipeWire Integration** — Automatic virtual source creation and cleanup
- **Auto-reconnect** — Watchdog detects connection loss and reconnects automatically
- **Settings Persistence** — Saves IP, ports, theme, and auto-connect to `~/.config/yumic/config.toml`
- **Theme Switcher** — System / Light / Dark

---

## Install

| Method | Command |
|--------|---------|
| **AUR** (Arch/Manjaro/CachyOS) | `yay -S yumic-server` |
| **AppImage** | Download from [Releases](https://github.com/Excez0/yumic-server/releases) |
| **Build from source** | See below |

### AUR

```bash
yay -S yumic-server
```

### AppImage

Download the `.AppImage` from [Releases](https://github.com/Excez0/yumic-server/releases), then:

```bash
chmod +x YuMic-*.AppImage
./YuMic-*.AppImage
```

### Build from Source

**Dependencies:**

| Distro | Command |
|--------|---------|
| Arch | `sudo pacman -S --needed rustup opus libadwaita pkg-config` |
| Fedora | `sudo dnf install cargo opus-devel libadwaita-devel pkgconf-pkg-config` |
| Ubuntu/Debian | `sudo apt install cargo rustc libopus-dev libadwaita-1-dev pkg-config` |

**Build & install:**

```bash
git clone https://github.com/Excez0/yumic-server.git
cd yumic-server
./install.sh
```

This compiles in `--release` mode, installs the binary to `~/.local/bin/`, and adds a desktop entry with icon.

### Firewall

YuMic uses **UDP port 49152** for audio. Allow it in your firewall:

```bash
# ufw
sudo ufw allow 49152/udp

# firewalld
sudo firewall-cmd --permanent --add-port=49152/udp && sudo firewall-cmd --reload
```

---

## Usage

1. Open **WO Mic** on your phone, set transport to **Wi-Fi**, and tap play. Note the IP address.
2. Launch **YuMic** on Linux (from app menu or `yumic-server` in terminal).
3. Enter your phone's IP and click **Connect**.
4. Select **YuMic_Microphone** in Discord, OBS, Zoom, or your system audio settings.

---

## Uninstall

```bash
./uninstall.sh
```

---

## WO Mic Protocol

YuMic implements the binary WO Mic protocol from scratch:

**TCP Handshake (port 8125):**
| Cmd | Byte | Description |
|-----|------|-------------|
| Hello | `0x65` | Client sends `[04, 04, 06, 02, 00, 00]`, phone echoes back |
| Set Media | `0x66` | Client sends UDP port as big-endian u16 |
| Start | `0x67` | Client requests stream start |
| Heartbeat | `0x69` | Sent every 1s to keep connection alive |

**UDP Audio (port 49152):**
- 11-byte header followed by Opus payload
- Byte 11 (`0xF8`): Opus TOC — CELT 48kHz, 20ms frame, mono
- Stripping 11 bytes yields a clean Opus frame

---

## Contributing

Contributions are welcome! Open an issue for bugs or feature requests, or submit a pull request.

---

## Support

If you find YuMic useful, consider supporting development:

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-excez-yellow?style=for-the-badge&logo=buycoffeesmile&logoColor=black)](https://buymeacoffee.com/excez)

---

## License

[MIT](LICENSE)
