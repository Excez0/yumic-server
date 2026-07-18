<p align="center">
  <img src="assets/yumic.svg" alt="YuMic Logo" width="128" height="128"/>
</p>

<h1 align="center">📱 YuMic</h1>

<p align="center">
  <strong>Native Linux client for WO Mic (Android & iOS) with a gorgeous GTK4 + Libadwaita interface.</strong>
</p>

<p align="center">
  <a href="https://github.com/Excez0/yumic-server/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/Excez0/yumic-server?style=flat-square&color=blue" alt="License"/>
  </a>
  <img src="https://img.shields.io/badge/platform-Linux-blue?style=flat-square&color=3584e4" alt="Platform Linux"/>
  <img src="https://img.shields.io/badge/language-Rust-orange?style=flat-square" alt="Language Rust"/>
  <img src="https://img.shields.io/badge/GUI-GTK4%20%2F%20Libadwaita-26a269?style=flat-square" alt="GUI GTK4/Libadwaita"/>
</p>

---

## ❓ Why YuMic? (Problem & Solution)

* **The Problem**: WO Mic is a fantastic app to turn your phone into a microphone, but its official Linux support is practically non-existent. The official CLI client is closed-source, hard to set up, requires obsolete ALSA loopback kernel modules that often crash, and offers no visual interface or feedback.
* **The Solution**: **YuMic** is a modern, open-source, native Linux client written in Rust. It offers a clean GTK4 + Libadwaita UI, live audio level meters, automated virtual microphone routing via PipeWire, and minimal CPU usage.

---

## ✨ Features

- **✅ Real-time Opus Decoding**: Zero-copy decoding utilizing the highly optimized Opus codec (48kHz, mono).
- **✅ Gorgeous GTK4 + Libadwaita UI**: Seamless integration with modern desktop environments (GNOME, KDE Plasma, etc.).
- **✅ Custom Cairo Audio Meter**: A custom pixel-drawn, gradient-colored (Green → Yellow → Orange → Red) audio visualizer with peak-hold tracking.
- **✅ Background Running (Minimize to Tray)**: Closing the window hides it to the system tray using pure D-Bus SNI (`ksni`). Clicking the tray icon or using the right-click menu toggles window visibility.
- **✅ PipeWire Integration**: Automated, safe creation and deletion of the virtual source without blocking your audio server.
- **✅ Configurable Settings**: Save/load phone IP, custom TCP/UDP ports, dark/light theme choice, and auto-connect preferences (`~/.config/yumic/config.toml`).

---

## 🛠️ Installation

### 1. Install Dependencies

Before compiling, install the required development libraries for your distribution:

* **Arch Linux**:
  ```bash
  sudo pacman -S --needed rustup opus libadwaita pkg-config
  ```
* **Fedora**:
  ```bash
  sudo dnf install cargo opus-devel libadwaita-devel pkgconf-pkg-config
  ```
* **Ubuntu / Debian**:
  ```bash
  sudo apt install cargo rustc libopus-dev libadwaita-1-dev pkg-config
  ```

> ⚠️ **Firewall Notice (Important)**
> YuMic uses **UDP port 49152** for audio data. If you have a firewall (ufw, firewalld, iptables) enabled, you **must** allow this port, otherwise audio packets from your phone will not reach your computer.
>
> **UFW (Ubuntu/Debian/Arch):**
> ```bash
> sudo ufw allow 49152/udp
> sudo ufw reload
> ```
> **firewalld (Fedora/RHEL):**
> ```bash
> sudo firewall-cmd --permanent --add-port=49152/udp
> sudo firewall-cmd --reload
> ```

### 2. Build & Install with One-Click Script

```bash
# Clone the repository
git clone https://github.com/Excez0/yumic-server.git
cd yumic-server

# Run the automated installer
./install.sh
```

The installer will compile the app in optimized `--release` mode, place the binary in `~/.local/bin/yumic-server`, and install the **YuMic** application launcher shortcut with its scalable SVG logo in your desktop applications menu.

---

## 🚀 Quick Start (How to Use)

1. **On your Phone**:
   - Open the **WO Mic** app (Android or iOS).
   - Go to settings and set **Transport** to `Wi-Fi`.
   - Start the server on the phone by tapping the play icon. Note the IP address displayed (e.g. `192.168.1.105`).

2. **On your Linux Desktop**:
   - Launch **YuMic** from your applications menu (or run `yumic-server` in terminal).
   - Enter your phone's IP address.
   - Click **Connect**.
   - Your phone is now streaming! Select the **YuMic_Microphone** virtual device in Discord, OBS, Zoom, or your system sound settings.

---

## 🗑️ Uninstallation

To completely remove YuMic, its launcher shortcut, and local icons from your system, run:
```bash
./uninstall.sh
```

---

## 🔬 Reverse-Engineered WO Mic Protocol

YuMic is built on a clean-room implementation of the binary WO Mic protocol:

1. **TCP Handshake (Port 8125)**:
   - **Hello (0x65)**: Sent by client with payload `[04, 04, 06, 02, 00, 00]` → Phone replies with echo.
   - **Set Media Port (0x66)**: Client sends the local UDP port (typically `49152`) as a big-endian u16.
   - **Start (0x67)**: Client requests the phone to start streaming.
   - **Heartbeat (0x69)**: Client must send a heartbeat packet every **1 second** to prevent connection timeout.

2. **UDP Audio Stream (Port 49152)**:
   - **WO Mic UDP Header (11 bytes)**:
     - Byte 0: `0x04` (Audio packet type identifier).
     - Bytes 1-2: `0x00 0x00` (Reserved).
     - Byte 3: Remaining packet size.
     - Bytes 4-5: 16-bit packet sequence counter.
     - Bytes 6-7: `0x00 0x00` (Reserved).
     - Bytes 8-9: 16-bit audio timestamp.
     - Byte 10: `0x05` (WO Mic flag).
   - **Opus Payload (Starts at Byte 11)**:
     - Byte 11 (`0xF8`): **Opus TOC byte**. According to RFC 6716, `0xF8` maps to **CELT-only mode, 48 kHz, 20 ms frame size, mono channel, 1 frame per packet**.
     - Stripping exactly **11 bytes** feeds the decoder with a clean Opus frame, yielding **0 decode errors** and crystal-clear audio.

---

## 🤝 Contributing

Contributions are welcome!
- If you find a bug or want to suggest a feature, please open an **Issue**.
- If you want to contribute code, feel free to fork the repository and submit a **Pull Request**.

---

## 📄 License

This project is licensed under the MIT License.
