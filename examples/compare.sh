#!/bin/bash
# Renders the native widget (harness.rs) and the original Python widget
# (python_reference.py) with identical options in a private headless KWin and
# compares every pair: PNGs pixel by pixel, the lines both print, the commands
# both ran and the files they left.
#
#   cargo build --lib --examples
#   examples/compare.sh                 # ONLY=REGEX limits the scenarios
#
# Nothing touches the real session: a private D-Bus session, HOME, TMPDIR and
# the XDG directories under .test-env, rebuilt before every run, and fake
# commands first on PATH that only log (and, for an install, leave what the
# real ones would): curl, the sudo wrapper, pacman, rum, sudo and pkexec. The
# configuration script in the scratch HOME is a fake too. The host and the
# Python stand-in accept only the password "secret".
set -u
examples=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$examples/.." && pwd)
env_dir=$repo/.test-env
out=$env_dir/compare
socket=${LINEXIN_TEST_SOCKET:-linexin-test-affinity}
# Where cargo built this checkout (CARGO_TARGET_DIR and cargo's settings
# count), asked before the private session, which has no cargo on PATH.
target=${LINEXIN_COMPARE_TARGET:-$(cd "$repo" && cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/debug}
# The original Python widget: installed, or the copy PYTHON_WIDGET names.
python_widget=${PYTHON_WIDGET:-/usr/share/linexin/widgets/d-affinity_installer.py}

if [ -z "${LINEXIN_AFFINITY_SESSION:-}" ]; then
    harness=$target/examples/harness
    library=$target/libaffinity_installer.so
    [ -x "$harness" ] && [ -f "$library" ] || { echo "run: cargo build --lib --examples" >&2; exit 2; }
    [ -f "$python_widget" ] || {
        echo "No original Python widget at $python_widget: install it, or name its file in PYTHON_WIDGET" >&2
        exit 2
    }
    rm -rf "$env_dir"
    mkdir -p "$out/png" "$out/logs" "$env_dir/bin" "$env_dir/widgets"
    # Installed layout: the library beside screenshots/.
    cp "$library" "$env_dir/widgets/d-affinity_installer.so"
    ln -s "$repo/data/usr/share/linexin/widgets/screenshots" "$env_dir/widgets/screenshots"

    cat >"$env_dir/bin/fake-wrapper" <<'EOF'
#!/bin/sh
echo "wrapper $*" >>"$FAKE_LOG"
exec "$@"
EOF
    cat >"$env_dir/bin/pacman" <<'EOF'
#!/bin/sh
echo "pacman $*" >>"$FAKE_LOG"
[ -n "${FAKE_PACMAN_SLEEP:-}" ] && sleep "$FAKE_PACMAN_SLEEP"
exit "${FAKE_PACMAN_STATUS:-0}"
EOF
    # rum wine-affinity PREFIX wine INSTALLER.exe "installs" $FAKE_RUM_APP.
    cat >"$env_dir/bin/rum" <<'EOF'
#!/bin/sh
echo "rum $*" >>"$FAKE_LOG"
[ -n "${FAKE_RUM_SLEEP:-}" ] && sleep "$FAKE_RUM_SLEEP"
status=${FAKE_RUM_STATUS:-0}
[ "$status" != 0 ] && exit "$status"
if [ "$#" = 4 ] && [ "$3" = wine ]; then
    case "$4" in
        */Program\ Files/*) ;;
        *.exe) mkdir -p "$2/drive_c/Program Files/Affinity/${FAKE_RUM_APP:-Photo 2}" ;;
    esac
fi
exit 0
EOF
    # The native widget downloads with curl (the original used urllib, whose
    # stand-in in python_reference.py logs the same line).
    cat >"$env_dir/bin/curl" <<'EOF'
#!/bin/sh
out=; url=
while [ $# -gt 0 ]; do
    case "$1" in
        --output) out=$2; shift ;;
        --write-out) shift ;;
        -*) ;;
        *) url=$1 ;;
    esac
    shift
done
echo "download $url -> $out" >>"$FAKE_LOG"
status=${FAKE_DOWNLOAD_STATUS:-200}
if [ "$status" != 200 ]; then
    printf '%s' "$status"
    echo "curl: (22) The requested URL returned error: $status" >&2
    exit 22
fi
printf '\r                                         0.0%%' >&2; sleep 0.1
printf '\r####################                    50.0%%' >&2; sleep 0.1
printf '\r######################################## 100.0%%\n' >&2; sleep 0.1
printf 'MZ' >"$out"
printf 200
EOF
    cat >"$env_dir/bin/fake" <<'EOF'
#!/bin/sh
echo "${0##*/} $*" >>"$FAKE_LOG"
exit 1
EOF
    chmod 755 "$env_dir/bin/"*
    for name in sudo pkexec xdg-open; do ln -s fake "$env_dir/bin/$name"; done
    cat >"$env_dir/bus.conf" <<'EOF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy>
</busconfig>
EOF
    "$harness" --print-css >"$env_dir/center-style.css" || exit 2
    exec env -i LINEXIN_COMPARE_TARGET="$target" PYTHON_WIDGET="$python_widget" \
        LINEXIN_CENTER_CSS="$env_dir/center-style.css" PATH="$env_dir/bin:/usr/bin" HOME="$env_dir/home" TMPDIR="$env_dir/tmp" \
        XDG_CONFIG_HOME="$env_dir/home/.config" XDG_CACHE_HOME="$env_dir/home/.cache" \
        XDG_DATA_HOME="$env_dir/home/.local/share" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
        LANG=en_US.UTF-8 ADW_DISABLE_PORTAL=1 GDK_DEBUG=no-portals GTK_A11Y=none \
        FAKE_WRAPPER="$env_dir/bin/fake-wrapper" FAKE_PASSWORD=secret \
        LINEXIN_AFFINITY_SESSION=1 ONLY="${ONLY:-}" \
        dbus-run-session --config-file="$env_dir/bus.conf" -- bash -c '
        kwin_wayland --virtual --no-lockscreen --width 1600 --height 1000 --socket "$1" \
            >"$2/logs/kwin.log" 2>&1 &
        kwin=$!
        for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/$1" ] && break; sleep 0.1; done
        WAYLAND_DISPLAY=$1 GDK_BACKEND=wayland "$3"
        status=$?
        kill $kwin; wait $kwin 2>/dev/null
        exit $status' session "$socket" "$out" "$0"
fi

harness=$target/examples/harness
library=$env_dir/widgets/d-affinity_installer.so
skel=$repo/data/etc/skel/.local/share/linexin
prefix_apps="$HOME/.WineAffinity/drive_c/Program Files/Affinity"
total=0
differing=0

# fixture APPS: a fresh HOME and TMPDIR as the package leaves them for a user
# (the configuration script still 0644 until the install's chmod), with the
# apps of APPS (";"-separated) installed and their menu entries in place.
fixture() {
    rm -rf "$HOME" "$TMPDIR"
    mkdir -p "$HOME/.config" "$HOME/.cache" "$HOME/.local/share/applications" \
        "$HOME/.local/share/linexin/scripts" "$HOME/.local/share/linexin/shortcuts" \
        "$HOME/Desktop" "$HOME/Downloads" "$TMPDIR"
    cp "$skel"/shortcuts/*.desktop "$HOME/.local/share/linexin/shortcuts/"
    cat >"$HOME/.local/share/linexin/scripts/wine-affinity-config.sh" <<'EOF'
#!/bin/sh
read -r answer
echo "config answered=$answer" >>"$FAKE_LOG"
exit "${FAKE_CONFIG_STATUS:-0}"
EOF
    chmod 644 "$HOME/.local/share/linexin/scripts/wine-affinity-config.sh"
    touch "$HOME/Desktop/Affinity.lnk"
    printf 'MZ' >"$HOME/Downloads/Affinity.exe"
    local app
    IFS=';' read -r -a installed <<<"$1"
    for app in "${installed[@]}"; do
        [ -n "$app" ] || continue
        mkdir -p "$prefix_apps/$app"
        local desktop
        case "$app" in
            Affinity) desktop=affinity.desktop ;;
            "Photo 2") desktop=photo-2.desktop ;;
            "Designer 2") desktop=designer-2.desktop ;;
            "Publisher 2") desktop=publisher-2.desktop ;;
        esac
        mv "$HOME/.local/share/linexin/shortcuts/$desktop" "$HOME/.local/share/applications/"
    done
}

# pair TAG APPS ENV OPTIONS...: both renderers in the same fixture and
# environment (ENV: VAR=value[,VAR=value]... or "-"); the PNG names in
# OPTIONS (@) are rewritten per side, into png/TAG/.
pair() {
    local tag=$1 apps=$2 vars=$3
    shift 3
    [ -n "$ONLY" ] && ! [[ $tag =~ $ONLY ]] && return
    local side extra=()
    mkdir -p "$out/png/$tag"
    [ "$vars" != - ] && IFS=',' read -r -a extra <<<"$vars"
    for side in rust py; do
        fixture "$apps"
        local args=() arg
        for arg in "$@"; do args+=("${arg//@/$out/png/$tag/$side}"); done
        local commands=$out/logs/$side-$tag.commands locks=$out/logs/$side-$tag.locks
        : >"$commands"
        : >"$locks"
        (
            cd "$HOME" || exit 1
            if [ $side = rust ]; then
                env "${extra[@]}" FAKE_LOG="$commands" FAKE_LOCK_LOG="$locks" \
                    timeout 90 "$harness" --library "$library" "${args[@]}"
            else
                env "${extra[@]}" FAKE_LOG="$commands" FAKE_LOCK_LOG="$locks" \
                    timeout 90 python3 -B "$examples/python_reference.py" "${args[@]}"
            fi
        ) >"$out/logs/$side-$tag.out" 2>"$out/logs/$side-$tag.err"
        echo "exit $?" >>"$out/logs/$side-$tag.out"
        sort -o "$locks" "$locks"
    done
    local differences=0 png kind
    if ! diff -q <(grep -v '^rendered' "$out/logs/rust-$tag.out" | grep -v '^widgetname') \
        <(grep -v '^rendered' "$out/logs/py-$tag.out" | grep -v '^widgetname') >/dev/null; then
        echo "$tag: printed lines differ"
        diff <(grep -v '^widgetname' "$out/logs/rust-$tag.out") <(grep -v '^widgetname' "$out/logs/py-$tag.out") | head -10
        differences=1
    fi
    for kind in commands locks; do
        if ! diff -q "$out/logs/rust-$tag.$kind" "$out/logs/py-$tag.$kind" >/dev/null; then
            echo "$tag: $kind differ"
            diff "$out/logs/rust-$tag.$kind" "$out/logs/py-$tag.$kind" | head -10
            differences=1
        fi
    done
    local pairs=0
    for png in "$out/png/$tag"/rust*.png; do
        [ -e "$png" ] || continue
        pairs=$((pairs + 1))
        local other=$out/png/$tag/py${png##*/rust}
        if ! python3 - "$png" "$other" <<'EOF'
import sys
from PIL import Image, ImageChops
a, b = (Image.open(p).convert("RGBA") for p in sys.argv[1:3])
if a.size != b.size:
    print(f"{sys.argv[1]}: size {a.size} vs {b.size}")
    sys.exit(1)
box = ImageChops.difference(a, b).getbbox()
if box:
    print(f"{sys.argv[1]}: differs in {box}")
    sys.exit(1)
EOF
        then
            differences=1
        fi
    done
    total=$((total + 1))
    if [ $differences = 0 ]; then
        echo "$tag: $pairs image pair(s), identical"
    else
        differing=$((differing + 1))
    fi
}

one="Photo 2"
many="Photo 2;Designer 2"
download=(--click download --password secret)
for lang in en_US pl_PL; do
    pair "$lang-fresh" "" - --lang $lang --output @.png
    pair "$lang-installed" "$one" - --lang $lang --output @.png
done
pair "dark-fresh" "" - --dark --output @.png
pair "dark-installed" "$one" - --dark --output @.png
pair "small" "" - --width 800 --height 600 --output @.png
pair "hide-sidebar" "" - --hide-sidebar --output @.png
pair "carousel-next" "" - --click next --after-click @.png
pair "carousel-prev" "" - --click prev --after-click @.png
pair "carousel-twice" "" - --click next --click next --after-click @.png
pair "split-menu" "" - --click menu --dialog-output @-menu.png
pair "password-dialog-pl" "" - --lang pl_PL --click download --dialog-output @-dialog.png
pair "wrong-password" "" - --click download --password nope --final-dialog-output @-dialog.png
pair "empty-password" "" - --click download --password "" --final-output @.png
pair "file-chooser" "" - --click select-exe --dialog-output @-chooser.png
pair "remove-dialog" "$one" - --click Remove --dialog-output @-dialog.png
pair "remove" "$many" - --click Remove --respond remove --wait-finish --final-output @.png
pair "choose-dialog" "$many" - --click Launch --dialog-output @-dialog.png
pair "launch" "$one" FAKE_RUM_SLEEP=2 --click Launch --after-click @-launching.png --wait-finish --final-output @.png
pair "launch-chosen" "$many" - --click Launch --respond "Designer 2" --wait-finish
pair "install-download" "" FAKE_PACMAN_SLEEP=2 "${download[@]}" \
    --progress-at "Step 2: Installing Wine..." --progress-output @-progress.png \
    --wait-finish --final-output @.png
pair "install-download-pl" "" FAKE_PACMAN_SLEEP=2 --lang pl_PL "${download[@]}" \
    --progress-at "Krok 2: Instalowanie Wine..." --progress-output @-progress.png --wait-finish
pair "install-exe" "" FAKE_RUM_SLEEP=2,FAKE_RUM_APP=Affinity --click select-exe \
    --choose "$HOME/Downloads/Affinity.exe" --password secret \
    --progress-at "Step 3: Installing Affinity..." --progress-output @-progress.png \
    --wait-finish --final-output @.png
pair "install-fails" "" FAKE_PACMAN_STATUS=1 "${download[@]}" --wait-finish \
    --final-dialog-output @-dialog.png --final-output @.png
pair "config-fails-pl" "" FAKE_CONFIG_STATUS=3 --lang pl_PL "${download[@]}" --wait-finish \
    --final-dialog-output @-dialog.png
pair "download-fails" "" FAKE_DOWNLOAD_STATUS=404 "${download[@]}" --wait-finish \
    --final-dialog-output @-dialog.png
pair "installer-fails" "" FAKE_RUM_STATUS=2 "${download[@]}" --wait-finish \
    --final-dialog-output @-dialog.png

echo "$total scenarios, $differing with differences"
[ $differing = 0 ]
