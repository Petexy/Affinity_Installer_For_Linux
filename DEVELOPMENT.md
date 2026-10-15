# Developing Affinity Installer

How this repository is built, tested and put together. What it is and how
to install and use it: [README.md](README.md).

The Affinity Installer widget of Linexin Center, rewritten in Rust as a native
widget (`d-affinity_installer.so`) for the [Rust Linexin Center](https://github.com/Petexy/Linexin-Center).
It replaces the Python `d-affinity_installer.py`: it installs Affinity Photo 2,
Designer 2, Publisher 2 or Affinity (2025) into a Wine prefix
(`~/.WineAffinity`) with the `wine-affinity` build, launches the installed apps
and removes them again.

It has two faces sharing the same steps (`widget/install.rs`):

- **GTK** (every desktop but LineXinBar, and LineXinBar without lxb-toolkit):
  identical to the Python widget — the header with Download and Install (and
  Select .exe file instead in its menu), the screenshot carousel, the password
  prompt (the Center's shared sudo helper; the Center is locked while pacman
  installs Wine), the progress page and its steps, Launch (with the Choose
  Application dialog when several apps are installed) and Remove.
- **LineXinBar**: a page in LineXinBar's own language, driven by controller,
  keyboard and pointer — rows for Download and Install, Select .exe file
  instead (the shell's file question), Launch and Remove (the shell's
  dialogs), the GTK page's screenshots as a picture over its thumbnails
  (beside the buttons in a landscape window, as wide as the window's height
  lets them stand, the page's margin between; under them otherwise), and the
  steps with a progress bar while installing. Wine is
  installed through `pkexec`, so the password is asked by the session's
  polkit agent: on LineXinBar, the shell's own panel with its on-screen
  keyboard.

The steps are the original's: download the installer (now with `curl`; failures
are still worded like urllib's, e.g. `HTTP Error 404: Not Found`), install
`wine-affinity` with pacman, run `~/.local/share/linexin/scripts/wine-affinity-config.sh`
answering `Y`, run the Affinity installer under `rum`, move the menu entries of
the installed apps from `~/.local/share/linexin/shortcuts`, and clean up. A
failure shows the same message the Python widget showed (`Command '...'
returned non-zero exit status 1.`, `Setting up the environment failed with exit
code 3.`).

## Build

The crate takes the Linexin Center's crates (`linexin-plugin-api`, `linexin-lxb`, `linexin-localization`)
from [its repository](https://github.com/Petexy/Linexin-Center), so a clone builds on its own. To build
against a local checkout of the Center instead, to change both at once, say so
in `.cargo/config.toml` (not committed):

```toml
[patch."https://github.com/Petexy/Linexin-Center"]
linexin-plugin-api = { path = "/path/to/Linexin-Center/crates/plugin-api" }
linexin-lxb = { path = "/path/to/Linexin-Center/crates/lxb" }
linexin-localization = { path = "/path/to/Linexin-Center/crates/localization" }
```

```sh
cargo build --release --lib
makepkg -si                 # the affinity-installer2 package
```

## Install layout

```text
/usr/share/linexin/widgets/d-affinity_installer.so
/usr/share/linexin/widgets/screenshots/affinity{1,2,3}.png
/usr/share/linexin/widgets/localization/<lang>/affinity_installer_dictionary.py
/usr/share/icons/github.petexy.affinityinstaller.png, ...
/usr/bin/{affinity,photo2,designer2,publisher2}, /usr/local/bin/rum
/etc/skel/.local/share/linexin/{scripts,shortcuts}/...
```

`data/` holds every file of the Python package except the widget itself, and
`affinity-installer2.install` is its install script, unchanged.

## Tests

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --lib --examples && examples/compare.sh
```

`examples/compare.sh` renders this widget (`examples/harness.rs`) and the
original Python widget (`examples/python_reference.py`) in a private headless
KWin and compares every pair pixel by pixel, with the lines both print, the
commands both ran and the files they left: English and Polish, dark mode,
small windows, the carousel, the split button's menu, the password, removal
and Choose Application dialogs, the file chooser, launching, and whole installs
(downloaded or chosen) that succeed or fail at each step. Nothing is installed:
the HOME is a scratch one, and curl, the sudo wrapper, pacman, rum, sudo and
pkexec are fakes first on `PATH`.

The comparison needs the original Python widget, installed where its package
puts it or named with `PYTHON_WIDGET=FILE`; it finds this checkout's build
wherever cargo put it (`CARGO_TARGET_DIR` counts), and the Center's stylesheet
comes with the Center's crates.

## Differences from the original

- If `rum` cannot be started, Launch comes back instead of staying at
  Launching... (the original's thread died there).
- The password is checked off the GTK thread, so the window does not freeze
  while sudo answers.
