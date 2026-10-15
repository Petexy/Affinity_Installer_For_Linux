#!/usr/bin/env python3
"""Run the ORIGINAL d-affinity_installer.py the way examples/harness.rs runs
the native port (same options, same scenario, same output lines), for
comparison.

The original Python Center injected `_`, `translate_dialog` and
`translate_window` into the widget module after loading it, using the
affinity_installer dictionaries; the functions below are its
`_translate_with_patterns_helper`, `_translate_dialog` and helpers, verbatim
apart from the minimal localization manager they read. Its subprocess patch,
which locked the window while a command through the sudo wrapper ran, is
reduced to logging. `sudo_manager` is a stand-in that accepts only
$FAKE_PASSWORD and logs what it is asked; the download is a stand-in for
urllib's that writes a two-byte file. Everything else the widget runs is a
fake command of compare.sh."""

import argparse
import importlib.util
import os
import re
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import warnings

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Graphene, Gtk  # noqa: E402

# PyGObject's notes about deprecated API (Adw.MessageDialog,
# Gtk.FileChooserDialog) are not the widget's output.
warnings.filterwarnings("ignore", category=DeprecationWarning)

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)

parser = argparse.ArgumentParser()
# The original widget: installed, or the copy PYTHON_WIDGET names.
parser.add_argument("--widget", default=os.environ.get(
    "PYTHON_WIDGET", "/usr/share/linexin/widgets/d-affinity_installer.py"))
parser.add_argument("--localization",
                    default=os.path.join(REPO, "data/usr/share/linexin/widgets/localization"))
# The Center's stylesheet: examples/harness --print-css writes it (compare.sh
# does, and names it in LINEXIN_CENTER_CSS).
parser.add_argument("--css", default=os.environ.get("LINEXIN_CENTER_CSS"))
parser.add_argument("--lang", default="en_US")
parser.add_argument("--dark", action="store_true")
parser.add_argument("--width", type=int, default=1300)
parser.add_argument("--height", type=int, default=820)
parser.add_argument("--hide-sidebar", action="store_true")
parser.add_argument("--output")
parser.add_argument("--click", action="append", default=[])
parser.add_argument("--after-click")
parser.add_argument("--dialog-output")
parser.add_argument("--choose")
parser.add_argument("--password")
parser.add_argument("--respond", default="cancel")
parser.add_argument("--progress-at")
parser.add_argument("--progress-output")
parser.add_argument("--wait-finish", action="store_true")
parser.add_argument("--final-output")
parser.add_argument("--final-dialog-output")
parser.add_argument("--library", help="ignored (harness.rs option)")
args = parser.parse_args()
if not args.css:
    parser.error("--css: the Center's stylesheet is needed (examples/harness --print-css >FILE)")

WIDGET_ID = "affinity_installer"
FALLBACK_LANGUAGE = "en_US"


# --- The original Center's localization, reduced to one domain -------------

class LocalizationManager:
    def __init__(self):
        self.system_language = args.lang
        self.widget_translations = {}
        if os.path.isdir(args.localization):
            for language_code in sorted(os.listdir(args.localization)):
                path = os.path.join(args.localization, language_code, f"{WIDGET_ID}_dictionary.py")
                if not os.path.isfile(path):
                    continue
                spec = importlib.util.spec_from_file_location(f"{WIDGET_ID}_{language_code}_dict", path)
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
                if hasattr(module, "translations"):
                    self.widget_translations.setdefault(WIDGET_ID, {})[language_code] = module.translations

    def get_text(self, key, widget_name="widget_loader"):
        if widget_name in self.widget_translations:
            widget_trans = self.widget_translations[widget_name]
            if self.system_language in widget_trans:
                if key in widget_trans[self.system_language]:
                    return widget_trans[self.system_language][key]
            if FALLBACK_LANGUAGE in widget_trans:
                if key in widget_trans[FALLBACK_LANGUAGE]:
                    return widget_trans[FALLBACK_LANGUAGE][key]
        return key


_manager = LocalizationManager()


def get_localization_manager():
    return _manager


def _(key, widget_name="widget_loader"):
    return get_localization_manager().get_text(key, widget_name)


def _translate_with_patterns_helper(text, widget_id):
    if not text:
        return ""
    translated = _(text, widget_id)
    if translated != text:
        return translated
    manager = get_localization_manager()
    if widget_id in manager.widget_translations:
        widget_trans = manager.widget_translations[widget_id]
        lang = manager.system_language
        trans_dict = widget_trans.get(lang) or widget_trans.get(FALLBACK_LANGUAGE, {})
        for pattern, translated_pattern in trans_dict.items():
            if '{}' not in pattern:
                continue
            escaped_pattern = re.escape(pattern)
            regex_pattern = escaped_pattern.replace(re.escape("{}"), r"(.*)")
            match = re.search(regex_pattern, text)
            if match and translated_pattern != pattern:
                if match.groups():
                    content = match.group(1)
                    translated_content = _translate_with_patterns_helper(content, widget_id)
                    translated_text = translated_pattern.replace("{}", translated_content, 1)
                    return text.replace(match.group(0), translated_text)
                else:
                    return text.replace(match.group(0), translated_pattern)
    return text


def _translate_dialog(dialog, widget_id=None):
    if widget_id is None:
        try:
            win = dialog.get_transient_for()
            if win and hasattr(win, 'active_widget_id'):
                widget_id = win.active_widget_id
        except Exception:
            pass
    if not widget_id:
        return
    if hasattr(dialog, 'get_heading') and dialog.get_heading():
        translated = _translate_with_patterns_helper(dialog.get_heading(), widget_id)
        if translated != dialog.get_heading():
            dialog.set_heading(translated)
    if hasattr(dialog, 'get_body') and dialog.get_body():
        translated = _translate_with_patterns_helper(dialog.get_body(), widget_id)
        if translated != dialog.get_body():
            dialog.set_body(translated)
    _translate_widget_recursive_static(dialog, widget_id)


def _translate_window(window, widget_id=None):
    if not widget_id:
        return
    _translate_widget_recursive_static(window, widget_id)


def _translate_widget_recursive_static(widget, widget_id):
    try:
        _translate_single_widget_static(widget, widget_id)
        if hasattr(widget, 'get_first_child'):
            child = widget.get_first_child()
            while child:
                _translate_widget_recursive_static(child, widget_id)
                child = child.get_next_sibling()
        elif hasattr(widget, 'get_child'):
            child = widget.get_child()
            if child:
                _translate_widget_recursive_static(child, widget_id)
    except Exception:
        pass


def _translate_single_widget_static(widget, widget_id):
    try:
        if isinstance(widget, Gtk.Label):
            if widget.get_use_markup():
                markup = widget.get_label()
                if markup:
                    translated_markup = _translate_markup_content_static(markup, widget_id)
                    if translated_markup != markup:
                        widget.set_markup(translated_markup)
            else:
                text = widget.get_text()
                if text:
                    translated = _translate_with_patterns_helper(text, widget_id)
                    if translated != text:
                        widget.set_text(translated)
        elif isinstance(widget, Gtk.Button):
            label = widget.get_label()
            if label:
                translated = _translate_with_patterns_helper(label, widget_id)
                if translated != label:
                    widget.set_label(translated)
        elif isinstance(widget, (Gtk.Entry, Gtk.SearchEntry, Gtk.PasswordEntry)):
            placeholder = widget.get_placeholder_text()
            if placeholder:
                translated = _translate_with_patterns_helper(placeholder, widget_id)
                if translated != placeholder:
                    widget.set_placeholder_text(translated)
        elif isinstance(widget, Adw.ActionRow):
            title = widget.get_title()
            if title:
                translated = _translate_with_patterns_helper(title, widget_id)
                if translated != title:
                    widget.set_title(translated)
            subtitle = widget.get_subtitle()
            if subtitle:
                translated = _translate_with_patterns_helper(subtitle, widget_id)
                if translated != subtitle:
                    widget.set_subtitle(translated)
        elif isinstance(widget, Adw.PreferencesGroup):
            title = widget.get_title()
            if title:
                translated = _translate_with_patterns_helper(title, widget_id)
                if translated != title:
                    widget.set_title(translated)
            desc = widget.get_description()
            if desc:
                translated = _translate_with_patterns_helper(desc, widget_id)
                if translated != desc:
                    widget.set_description(translated)
        elif isinstance(widget, (Adw.PreferencesRow, Adw.PreferencesPage)):
            title = widget.get_title()
            if title:
                translated = _translate_with_patterns_helper(title, widget_id)
                if translated != title:
                    widget.set_title(translated)
        elif isinstance(widget, Adw.StatusPage):
            title = widget.get_title()
            if title:
                translated = _translate_with_patterns_helper(title, widget_id)
                if translated != title:
                    widget.set_title(translated)
            desc = widget.get_description()
            if desc:
                translated = _translate_with_patterns_helper(desc, widget_id)
                if translated != desc:
                    widget.set_description(translated)
        elif isinstance(widget, (Gtk.Window, Adw.ApplicationWindow)):
            title = widget.get_title()
            if title:
                translated = _translate_with_patterns_helper(title, widget_id)
                if translated != title:
                    widget.set_title(translated)
    except Exception:
        pass


def _translate_markup_content_static(markup_text, widget_id):
    markup_text = _translate_with_patterns_helper(markup_text, widget_id)

    def replace_text(match):
        text_content = match.group(1)
        if text_content.strip():
            translated = _(text_content.strip(), widget_id)
            if translated != text_content.strip():
                leading_space = text_content[:len(text_content) - len(text_content.lstrip())]
                trailing_space = text_content[len(text_content.rstrip()):]
                return ">" + leading_space + translated + trailing_space + "<"
        return ">" + text_content + "<"
    return re.sub(r'>([^<]+)<', replace_text, markup_text)


# --- Stand-ins for the Center's services -----------------------------------

def log(variable, line):
    path = os.environ.get(variable)
    if path:
        with open(path, "a") as file:
            file.write(line + "\n")


class FakeSudo:
    user_password = None
    wrapper_path = os.environ.get("FAKE_WRAPPER", "/bin/false")

    def validate_password(self, password):
        log("FAKE_LOG", "sudo validate_password")
        expected = os.environ.get("FAKE_PASSWORD", "")
        return bool(expected) and password == expected

    def set_password(self, password):
        log("FAKE_LOG", "sudo set_password")

    def forget_password(self):
        log("FAKE_LOG", "sudo forget_password")

    def start_privileged_session(self):
        log("FAKE_LOG", "sudo start_privileged_session")

    def stop_privileged_session(self):
        log("FAKE_LOG", "sudo stop_privileged_session")

    def get_env(self):
        return os.environ.copy()


sudo = FakeSudo()
_original_popen = subprocess.Popen


class LockedPopen(_original_popen):
    """The original Center's _LockedPopen, logging instead of locking."""

    def __init__(self, *popen_args, **kwargs):
        cmd = popen_args[0] if popen_args else kwargs.get("args")
        if isinstance(cmd, str) and sudo.wrapper_path in cmd:
            log("FAKE_LOCK_LOG", "lock")
            super().__init__(*popen_args, **kwargs)

            def monitor():
                try:
                    self.wait()
                finally:
                    log("FAKE_LOCK_LOG", "unlock")
            threading.Thread(target=monitor, daemon=True).start()
        else:
            super().__init__(*popen_args, **kwargs)


subprocess.Popen = LockedPopen


def fake_urlretrieve(url, filename=None, reporthook=None, data=None):
    log("FAKE_LOG", f"download {url} -> {filename}")
    status = int(os.environ.get("FAKE_DOWNLOAD_STATUS", "200"))
    if status != 200:
        raise urllib.error.HTTPError(url, status, "Not Found" if status == 404 else "Error", None, None)
    for blocks in range(3):
        if reporthook:
            reporthook(blocks, 8192, 16384)
        time.sleep(0.1)
    with open(filename, "wb") as file:
        file.write(b"MZ")
    return filename, None


urllib.request.urlretrieve = fake_urlretrieve


# --- Load the widget module like the original _prepare_module ---------------

spec = importlib.util.spec_from_file_location("d-affinity_installer", args.widget)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def injected_underscore(message, widget_id=WIDGET_ID):
    return _translate_with_patterns_helper(message, widget_id)


module._ = injected_underscore
module.sudo_manager = sudo
module.translate_dialog = lambda dialog: _translate_dialog(dialog, WIDGET_ID)
module.translate_window = lambda window: _translate_window(window, WIDGET_ID)


# --- Scenario (mirrors examples/harness.rs) ----------------------------------

def q(text):
    """Rust's {:?} for the strings printed here."""
    return '"' + (text or "").replace("\\", "\\\\").replace('"', '\\"').replace(
        "\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + '"'


def b(value):
    return "true" if value else "false"


def ql(items):
    return "[" + ", ".join(q(item) for item in items) + "]"


def find_all(widget, kind, found):
    if isinstance(widget, kind):
        found.append(widget)
    child = widget.get_first_child()
    while child is not None:
        find_all(child, kind, found)
        child = child.get_next_sibling()
    return found


def render(widget, path):
    width, height = widget.get_width(), widget.get_height()
    snapshot = Gtk.Snapshot()
    Gtk.WidgetPaintable.new(widget).snapshot(snapshot, width, height)
    node = snapshot.to_node()
    renderer = widget.get_native().get_renderer()
    viewport = Graphene.Rect().init(0, 0, width, height)
    renderer.render_texture(node, viewport).save_to_png(path)
    print(f"rendered {width}x{height} to {path}")


def iterate(seconds):
    context = GLib.MainContext.default()
    until_time = time.monotonic() + seconds
    while time.monotonic() < until_time:
        while context.pending():
            context.iteration(False)
        time.sleep(0.002)


def until(what, ready):
    start = time.monotonic()
    while not ready():
        if time.monotonic() - start > PATIENCE:
            raise RuntimeError(f"timed out waiting for {what}")
        iterate(0.05)


SETTLE = 1.5
PATIENCE = 30


def message_dialogs():
    return [w for w in Gtk.Window.list_toplevels()
            if isinstance(w, Adw.MessageDialog) and w.get_visible()]


def file_choosers():
    return [w for w in Gtk.Window.list_toplevels()
            if isinstance(w, Gtk.FileChooserDialog) and w.get_visible()]


def print_dialogs():
    for dialog in message_dialogs():
        print(f"dialog heading={q(dialog.get_heading())} body={q(dialog.get_body())}")
    for chooser in file_choosers():
        current = chooser.get_filter()
        print(f"chooser title={q(chooser.get_title())} "
              f"filter={q(current.get_name() if current else '')} modal={b(chooser.get_modal())}")


def action_box(widget):
    return widget.get_first_child().get_last_child()


def stack(widget):
    return widget.get_last_child()


def describe(widget):
    actions = []
    child = action_box(widget).get_first_child()
    while child is not None:
        classes = [c for c in ("suggested-action", "destructive-action") if child.has_css_class(c)]
        if isinstance(child, Adw.SplitButton):
            actions.append(f"split {q(child.get_label())} sensitive={b(child.get_sensitive())} {ql(classes)}")
        elif isinstance(child, Gtk.Button):
            actions.append(f"button {q(child.get_label())} sensitive={b(child.get_sensitive())} {ql(classes)}")
        child = child.get_next_sibling()
    page = stack(widget).get_visible_child_name() or ""
    line = f"state page={page} actions=[{', '.join(actions)}]"
    if page == "progress":
        labels = find_all(stack(widget), Gtk.Label, [])
        bars = find_all(stack(widget), Gtk.ProgressBar, [])
        step = next((label.get_label() for label in labels if label.has_css_class("title-4")), "")
        fraction = bars[0].get_fraction() if bars else -1.0
        line += f" step={q(step)} fraction={fraction:.4f}"
    return line


def step_label(widget):
    line = describe(widget)
    if " step=" not in line:
        return ""
    return line.split(" step=")[1].split(" fraction=")[0].strip('"')


def print_files():
    home = os.environ.get("HOME", "")
    temporary = os.environ.get("TMPDIR", "")

    def listing(path):
        try:
            return sorted(os.listdir(path))
        except OSError:
            return []
    print(f"files apps={ql(listing(os.path.join(home, '.WineAffinity/drive_c/Program Files/Affinity')))}"
          f" applications={ql(listing(os.path.join(home, '.local/share/applications')))}"
          f" shortcuts={ql(listing(os.path.join(home, '.local/share/linexin/shortcuts')))}"
          f" desktop={ql(listing(os.path.join(home, 'Desktop')))}"
          f" tmp={ql(listing(temporary))}")


def click(widget, what):
    if what in ("next", "prev"):
        icon = "go-next-symbolic" if what == "next" else "go-previous-symbolic"
        buttons = [button for button in find_all(widget, Gtk.Button, [])
                   if button.get_icon_name() == icon]
        if not buttons:
            raise RuntimeError(f"no {what} button")
        buttons[0].emit("clicked")
    elif what in ("download", "menu"):
        splits = find_all(widget, Adw.SplitButton, [])
        if not splits:
            raise RuntimeError("no split button")
        if what == "download":
            splits[0].emit("clicked")
        else:
            splits[0].popup()
    elif what == "select-exe":
        if not widget.activate_action("install.select_exe", None):
            raise RuntimeError("no install.select_exe action")
    else:
        buttons = [button for button in find_all(action_box(widget), Gtk.Button, [])
                   if button.get_label() == what]
        if not buttons:
            raise RuntimeError(f"no button {what!r}")
        buttons[0].emit("clicked")
    print(f"clicked {what}")


def scenario(window, widget):
    iterate(SETTLE)
    print(describe(widget))
    if args.output:
        render(window, args.output)
    if not args.click:
        print_files()
        return
    for what in args.click:
        click(widget, what)
        iterate(0.3)
    iterate(SETTLE)
    print(describe(widget))
    print_dialogs()
    if args.after_click:
        render(window, args.after_click)
    if args.dialog_output:
        if message_dialogs():
            render(message_dialogs()[0], args.dialog_output)
        elif file_choosers():
            render(file_choosers()[0], args.dialog_output)
        else:
            popovers = [p for p in find_all(widget, Gtk.PopoverMenu, []) if p.get_visible()]
            if popovers:
                render(popovers[0], args.dialog_output)
    if args.choose:
        choosers = file_choosers()
        if not choosers:
            raise RuntimeError("no file chooser")
        choosers[0].set_file(Gio.File.new_for_path(args.choose))
        iterate(0.5)
        choosers[0].response(Gtk.ResponseType.OK)
        iterate(SETTLE)
        print(f"chose {args.choose}")
        print_dialogs()
    dialogs = message_dialogs()
    if args.password is not None:
        if not dialogs:
            raise RuntimeError("no password dialog")
        entries = find_all(dialogs[0], Gtk.PasswordEntry, [])
        if not entries:
            raise RuntimeError("no password entry")
        entries[0].set_text(args.password)
        dialogs[0].response("unlock")
        print("responded unlock")
    else:
        for dialog in dialogs:
            dialog.response(args.respond)
        for chooser in file_choosers():
            chooser.response(Gtk.ResponseType.CANCEL)
        print(f"responded {args.respond}")
    if args.progress_at:
        until(args.progress_at, lambda: step_label(widget) == args.progress_at)
        iterate(0.5)
        print(describe(widget))
        if args.progress_output:
            render(window, args.progress_output)
    if args.wait_finish:
        def done():
            line = describe(widget)
            return "page=progress" not in line and "sensitive=false" not in line
        until("the end", done)
    iterate(SETTLE)
    print(describe(widget))
    print_dialogs()
    if args.final_dialog_output and message_dialogs():
        render(message_dialogs()[0], args.final_dialog_output)
    if args.final_output:
        render(window, args.final_output)
    for dialog in message_dialogs():
        dialog.response("ok")
    print_files()


failed = False


def activate(app):
    global failed
    Adw.StyleManager.get_default().set_color_scheme(
        Adw.ColorScheme.FORCE_DARK if args.dark else Adw.ColorScheme.FORCE_LIGHT)
    css = Gtk.CssProvider()
    with open(args.css) as style:
        css.load_from_string(style.read())
    Gtk.StyleContext.add_provider_for_display(
        Gdk.Display.get_default(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    window = Adw.ApplicationWindow(application=app)
    window.set_default_size(args.width, args.height)
    widget = module.AffinityInstallerWidget(hide_sidebar=args.hide_sidebar, window=window)
    print(f"widgetname={widget.widgetname!r} icon={widget.widgeticon} id={WIDGET_ID}")
    window.set_content(widget)
    window.present()

    def run():
        global failed
        try:
            scenario(window, widget)
        except Exception as error:  # noqa: BLE001
            print(f"reference: {error}", file=sys.stderr)
            failed = True
        app.quit()
        return False

    GLib.idle_add(run)


app = Adw.Application(flags=Gio.ApplicationFlags.NON_UNIQUE)
app.connect("activate", activate)
app.run([])
sys.exit(1 if failed else 0)
