# Affinity Installer for Linux

<p align="center">
  <img src="data/usr/share/icons/github.petexy.affinityinstaller.png" alt="Affinity Installer" width="200" height="200"/>
</p>

**Affinity Installer** runs the **Affinity** creative apps on Linexin. It is a
widget for [Linexin Center](https://github.com/Petexy/Linexin-Center) that sets
up a Wine environment made for Affinity, installs Affinity (2025), Affinity
Photo 2, Designer 2 or Publisher 2 into it, and adds them to your application
menu.

This version is a rewrite in **Rust** (GTK4 + Libadwaita) and a drop-in
replacement for the Python widget: same package, same steps, same look.

## Supported distros:
- Linexin

## Distros supported after tinkering:
- Every Arch-based distro

## ✨ Features

* **Download and Install:** Downloads the Affinity (2025) installer for you and
  installs it, no browser needed.
* **Your Own Installer:** Already have an installer for Affinity Photo 2,
  Designer 2 or Publisher 2? Pick its `.exe` file instead.
* **Ready-Made Wine Setup:** Installs the `wine-affinity` build and prepares a
  Wine prefix (`~/.WineAffinity`) just for Affinity.
* **In Your Menu:** The installed apps appear in your application launcher.
* **Launch and Remove:** Start any installed Affinity app, or remove them, from
  the same page.
* **LineXinBar Ready:** On a LineXinBar desktop the installer is a
  controller-friendly page; the password is asked through the desktop's own
  prompt.

## How to use (on Linexin):
The app is already pre-installed on your distro.

1. Run "Affinity Installer" from your Linexin Center
2. Press "Download and Install" (or "Select .exe file instead" in its menu to
   use an Affinity installer you downloaded yourself)
3. Enter your password when asked, then wait and follow the Affinity installer
4. Run your program from the Application Launcher (or press "Launch")

## How to use (on Arch-based):

1. Add linexin-repo to your pacman.conf file in /etc/pacman.conf and update
   repos by using `pacman -Sy` (the `wine-affinity` package comes from there)
2. Use pacman to install the `affinity-installer2` and `linexin-center` packages
   and all their dependencies
3. Run Linexin Center and the Affinity Installer module
4. Follow the steps for Linexin above

## 🛠️ Requirements

* Linexin Center 4.0 or newer
* GTK 4 and Libadwaita
* `curl`
* `wine-affinity` (installed by the app itself, from linexin-repo)

## Troubleshooting:
- If the download fails, check your internet connection, or download the
  installer yourself and use "Select .exe file instead".
- If a step fails, the installer shows the error the command reported.

## 🔧 Building from Source

```bash
cargo build --release --lib
makepkg -si
```

Cargo fetches Linexin Center's crates from its repository while building.
Tests, how to build against a local Linexin Center checkout and how the widget
works inside are described in [DEVELOPMENT.md](DEVELOPMENT.md).

## Licence

Copyright (C) 2026 Petexy

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. It is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU General Public License in `LICENSE` for more
details.
