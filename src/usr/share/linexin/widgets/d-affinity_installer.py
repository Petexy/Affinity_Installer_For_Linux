#!/usr/bin/env python3
import gi
import subprocess
import threading
import gettext
import locale
import os
import shutil
import shlex
import glob
import tempfile
import urllib.request

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GLib, Gio, Gdk

APP_NAME = "affinity-installer"
LOCALE_DIR = "/usr/share/locale"

try:
    locale.setlocale(locale.LC_ALL, '')
    lang, encoding = locale.getlocale()
    if lang is None:
        lang = 'en'
except locale.Error:
    lang = 'en'

try:
    _
except NameError:
    gettext.bindtextdomain(APP_NAME, LOCALE_DIR)
    gettext.textdomain(APP_NAME)
    translation = gettext.translation(APP_NAME, LOCALE_DIR, languages=[lang], fallback=True)
    translation.install()
    _ = gettext.gettext

DOWNLOAD_URL = "https://downloads.affinity.studio/Affinity%20x64.exe"
SCREENSHOTS_DIR = "/usr/share/linexin/widgets/screenshots"
WINE_PREFIX = os.path.expanduser("~/.WineAffinity")
AFFINITY_DIR = os.path.join(WINE_PREFIX, "drive_c", "Program Files", "Affinity")

APP_REGISTRY = {
    "Affinity": {
        "folder": "Affinity",
        "exe": "Affinity.exe",
        "desktop": "affinity.desktop",
    },
    "Photo 2": {
        "folder": "Photo 2",
        "exe": "Photo.exe",
        "desktop": "photo-2.desktop",
    },
    "Designer 2": {
        "folder": "Designer 2",
        "exe": "Designer.exe",
        "desktop": "designer-2.desktop",
    },
    "Publisher 2": {
        "folder": "Publisher 2",
        "exe": "Publisher.exe",
        "desktop": "publisher-2.desktop",
    },
}


class AffinityInstallerWidget(Gtk.Box):
    def __init__(self, hide_sidebar=False, window=None):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.widgetname = _("Affinity Installer")
        self.widgeticon = "/usr/share/icons/github.petexy.affinityinstaller.png"
        self.set_margin_top(12)
        self.set_margin_bottom(48)
        self.set_margin_start(48)
        self.set_margin_end(48)

        self.window = window
        self.hide_sidebar = hide_sidebar
        self.install_started = False
        self.error_message = None
        self.user_password = None
        self.progress_data = ""
        self.total_steps = 6
        self.current_screenshot_idx = 0
        self.screenshots = self._load_screenshots()

        self._apply_css()
        self._build_header()
        self._build_content_stack()

        if self._is_installed():
            self._set_state_post_install()
        else:
            self._set_state_pre_install()

        if hide_sidebar:
            GLib.idle_add(self._resize_window)

    # ── Initialization helpers ──────────────────────────────────────

    def _resize_window(self):
        if self.window:
            try:
                self.window.set_default_size(800, 600)
            except Exception:
                pass
        return False

    @staticmethod
    def _load_screenshots():
        pattern = os.path.join(SCREENSHOTS_DIR, "affinity*.png")
        return sorted(glob.glob(pattern))

    def _apply_css(self):
        css = b"""
.screenshot-bg {
    background: linear-gradient(135deg, #4a1a8a, #0d021e, #3a1070);
    border-radius: 12px;
    padding: 24px;
    min-height: 300px;
}
"""
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(),
            provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
        )

    # ── UI construction ─────────────────────────────────────────────

    def _build_header(self):
        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        header.set_margin_bottom(12)

        icon = Gtk.Image()
        if os.path.exists(self.widgeticon):
            icon.set_from_file(self.widgeticon)
        else:
            icon.set_from_icon_name("package-x-generic")
        icon.set_pixel_size(48)
        header.append(icon)

        text_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        text_box.set_valign(Gtk.Align.CENTER)
        text_box.set_hexpand(True)

        title = Gtk.Label(label=_("Affinity Installer"))
        title.add_css_class("title-3")
        title.set_halign(Gtk.Align.START)
        text_box.append(title)

        subtitle = Gtk.Label(
            label=_("Supports Affinity Photo 2, Designer 2, Publisher 2 and Affinity (2025)")
        )
        subtitle.add_css_class("dim-label")
        subtitle.set_halign(Gtk.Align.START)
        subtitle.set_wrap(True)
        text_box.append(subtitle)

        header.append(text_box)

        self.action_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.action_box.set_valign(Gtk.Align.CENTER)
        header.append(self.action_box)

        self.append(header)
        self.append(Gtk.Separator(orientation=Gtk.Orientation.HORIZONTAL))

    def _build_content_stack(self):
        self.content_stack = Gtk.Stack()
        self.content_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.content_stack.set_hexpand(True)
        self.content_stack.set_vexpand(True)

        self._build_carousel()
        self._build_progress()

        self.append(self.content_stack)

    def _build_carousel(self):
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0)
        box.set_valign(Gtk.Align.FILL)
        box.set_vexpand(True)
        box.set_margin_top(24)
        box.set_margin_bottom(16)

        self.btn_prev = Gtk.Button.new_from_icon_name("go-previous-symbolic")
        self.btn_prev.add_css_class("flat")
        self.btn_prev.add_css_class("circular")
        self.btn_prev.set_valign(Gtk.Align.CENTER)
        self.btn_prev.connect("clicked", lambda _b: self._navigate_screenshot(-1))
        box.append(self.btn_prev)

        bg = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        bg.add_css_class("screenshot-bg")
        bg.set_hexpand(True)
        bg.set_vexpand(True)
        bg.set_valign(Gtk.Align.FILL)

        self.screenshot_picture = Gtk.Picture()
        self.screenshot_picture.set_can_shrink(True)
        try:
            self.screenshot_picture.set_content_fit(Gtk.ContentFit.CONTAIN)
        except AttributeError:
            pass
        self.screenshot_picture.set_hexpand(True)
        self.screenshot_picture.set_vexpand(True)
        bg.append(self.screenshot_picture)

        box.append(bg)

        self.btn_next = Gtk.Button.new_from_icon_name("go-next-symbolic")
        self.btn_next.add_css_class("flat")
        self.btn_next.add_css_class("circular")
        self.btn_next.set_valign(Gtk.Align.CENTER)
        self.btn_next.connect("clicked", lambda _b: self._navigate_screenshot(1))
        box.append(self.btn_next)

        self._show_screenshot(0)
        self.content_stack.add_named(box, "carousel")

    def _build_progress(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        box.set_valign(Gtk.Align.CENTER)
        box.set_halign(Gtk.Align.CENTER)
        box.set_vexpand(True)
        box.set_hexpand(True)
        box.set_margin_start(60)
        box.set_margin_end(60)
        box.set_margin_top(40)
        box.set_margin_bottom(40)

        self.step_label = Gtk.Label()
        self.step_label.add_css_class("title-4")
        box.append(self.step_label)

        self.progress_bar = Gtk.ProgressBar()
        self.progress_bar.set_hexpand(True)
        box.append(self.progress_bar)

        self.warning_label = Gtk.Label(
            label=_("Installation in progress. Do NOT close the app.")
        )
        self.warning_label.add_css_class("title-3")
        box.append(self.warning_label)

        spacer = Gtk.Box()
        spacer.set_vexpand(True)
        box.append(spacer)

        note = Gtk.Label()
        note.set_markup(
            _("The app will show you a couple of windows.")
            + "\n"
            + _(
                "All of those components need to be installed manually using "
                "buttons on the screen in order for the app to work properly"
            )
        )
        note.set_justify(Gtk.Justification.CENTER)
        note.set_wrap(True)
        note.add_css_class("dim-label")
        box.append(note)

        self.content_stack.add_named(box, "progress")

    # ── Screenshot carousel ─────────────────────────────────────────

    def _show_screenshot(self, index):
        if not self.screenshots:
            self.btn_prev.set_visible(False)
            self.btn_next.set_visible(False)
            return
        self.current_screenshot_idx = index % len(self.screenshots)
        self.screenshot_picture.set_filename(
            self.screenshots[self.current_screenshot_idx]
        )
        has_many = len(self.screenshots) > 1
        self.btn_prev.set_sensitive(has_many)
        self.btn_next.set_sensitive(has_many)

    def _navigate_screenshot(self, delta):
        self._show_screenshot(self.current_screenshot_idx + delta)

    # ── State management ────────────────────────────────────────────

    def _clear_actions(self):
        child = self.action_box.get_first_child()
        while child:
            nxt = child.get_next_sibling()
            self.action_box.remove(child)
            child = nxt

    def _set_state_pre_install(self):
        self._clear_actions()

        menu = Gio.Menu()
        menu.append(_("Select .exe file instead"), "install.select_exe")

        split = Adw.SplitButton()
        split.set_label(_("Download and Install"))
        split.set_menu_model(menu)
        split.add_css_class("suggested-action")
        split.connect("clicked", self._on_download_install)

        group = Gio.SimpleActionGroup()
        action = Gio.SimpleAction.new("select_exe", None)
        action.connect("activate", self._on_select_exe)
        group.add_action(action)
        self.insert_action_group("install", group)

        self.action_box.append(split)
        self.content_stack.set_visible_child_name("carousel")

    def _set_state_installing(self):
        self._clear_actions()
        btn = Gtk.Button(label=_("Installing..."))
        btn.set_sensitive(False)
        self.action_box.append(btn)
        self.content_stack.set_visible_child_name("progress")

    def _set_state_post_install(self):
        self._clear_actions()

        self.btn_remove = Gtk.Button(label=_("Remove"))
        self.btn_remove.add_css_class("destructive-action")
        self.btn_remove.connect("clicked", self._on_remove)
        self.action_box.append(self.btn_remove)

        self.btn_launch = Gtk.Button(label=_("Launch"))
        self.btn_launch.add_css_class("suggested-action")
        self.btn_launch.connect("clicked", self._on_launch)
        self.action_box.append(self.btn_launch)

        self.content_stack.set_visible_child_name("carousel")

    # ── Installation detection ──────────────────────────────────────

    @staticmethod
    def _is_installed():
        if not os.path.isdir(AFFINITY_DIR):
            return False
        return any(
            os.path.isdir(os.path.join(AFFINITY_DIR, a["folder"]))
            for a in APP_REGISTRY.values()
        )

    @staticmethod
    def _get_installed_apps():
        return [
            (name, info)
            for name, info in APP_REGISTRY.items()
            if os.path.isdir(os.path.join(AFFINITY_DIR, info["folder"]))
        ]

    # ── Action handlers ─────────────────────────────────────────────

    def _on_download_install(self, _btn):
        self._prompt_password(exe_path=None)

    def _on_select_exe(self, _action, _param):
        parent = self.get_root()
        dlg = Gtk.FileChooserDialog(
            title=_("Select Affinity .exe file"),
            transient_for=parent,
            modal=True,
            action=Gtk.FileChooserAction.OPEN,
        )
        dlg.add_button(_("Cancel"), Gtk.ResponseType.CANCEL)
        dlg.add_button(_("Open"), Gtk.ResponseType.OK)

        flt = Gtk.FileFilter()
        flt.set_name(_("Affinity Installer (*.exe)"))
        flt.add_pattern("*.exe")
        dlg.add_filter(flt)

        def on_resp(d, r):
            if r == Gtk.ResponseType.OK:
                f = d.get_file()
                if f:
                    self._prompt_password(exe_path=f.get_path())
            d.destroy()

        dlg.connect("response", on_resp)
        dlg.present()

    def _on_remove(self, _btn):
        dlg = Adw.MessageDialog(
            heading=_("Are you sure?"),
            body=_("This will remove Affinity and all its data from your system."),
            transient_for=self.get_root() or self.window,
        )
        dlg.add_response("cancel", _("Cancel"))
        dlg.add_response("remove", _("Remove"))
        dlg.set_response_appearance("remove", Adw.ResponseAppearance.DESTRUCTIVE)

        def on_resp(d, r):
            d.close()
            if r == "remove":
                self._perform_removal()

        dlg.connect("response", on_resp)
        try:
            translate_dialog(dlg)
        except NameError:
            pass
        dlg.present()

    def _on_launch(self, _btn):
        apps = self._get_installed_apps()
        if not apps:
            return
        if len(apps) == 1:
            self._launch_app(apps[0][1])
            return

        dlg = Adw.MessageDialog(
            heading=_("Choose Application"),
            body=_("Select which Affinity app to launch:"),
            transient_for=self.get_root() or self.window,
        )
        dlg.add_response("cancel", _("Cancel"))
        for name, info in apps:
            dlg.add_response(name, name)
            dlg.set_response_appearance(name, Adw.ResponseAppearance.SUGGESTED)

        def on_resp(d, r):
            d.close()
            if r != "cancel":
                for name, info in apps:
                    if r == name:
                        self._launch_app(info)
                        break

        dlg.connect("response", on_resp)
        try:
            translate_dialog(dlg)
        except NameError:
            pass
        dlg.present()

    def _set_buttons_launching(self):
        self.btn_launch.set_label(_("Launching..."))
        self.btn_launch.set_sensitive(False)
        self.btn_remove.set_sensitive(False)

    def _set_buttons_idle(self):
        self.btn_launch.set_label(_("Launch"))
        self.btn_launch.set_sensitive(True)
        self.btn_remove.set_sensitive(True)

    # ── Launch / Remove helpers ─────────────────────────────────────

    def _launch_app(self, app_info):
        exe = os.path.join(AFFINITY_DIR, app_info["folder"], app_info["exe"])
        self._set_buttons_launching()

        def _wait_for_exit():
            proc = subprocess.Popen(
                ["rum", "wine-affinity", WINE_PREFIX, "wine", exe],
                start_new_session=True,
            )
            proc.wait()
            GLib.idle_add(self._set_buttons_idle)

        threading.Thread(target=_wait_for_exit, daemon=True).start()

    def _perform_removal(self):
        self._clear_actions()
        btn = Gtk.Button(label=_("Removing..."))
        btn.set_sensitive(False)
        self.action_box.append(btn)

        def _remove():
            apps_dir = os.path.expanduser("~/.local/share/applications")
            for info in APP_REGISTRY.values():
                p = os.path.join(apps_dir, info["desktop"])
                if os.path.exists(p):
                    try:
                        os.remove(p)
                    except OSError:
                        pass
            if os.path.isdir(WINE_PREFIX):
                shutil.rmtree(WINE_PREFIX, ignore_errors=True)
            GLib.idle_add(self._set_state_pre_install)

        threading.Thread(target=_remove, daemon=True).start()

    # ── Password prompt ─────────────────────────────────────────────

    def _prompt_password(self, exe_path):
        dlg = Adw.MessageDialog(
            heading=_("Authentication Required"),
            body=_("Please enter your password to proceed with the installation."),
            transient_for=self.get_root() or self.window,
        )
        dlg.add_response("cancel", _("Cancel"))
        dlg.add_response("unlock", _("Unlock"))
        dlg.set_response_appearance("unlock", Adw.ResponseAppearance.SUGGESTED)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        entry = Gtk.PasswordEntry()
        entry.set_property("placeholder-text", _("Password"))
        box.append(entry)
        dlg.set_extra_child(box)

        def on_resp(d, r):
            d.close()
            if r == "unlock":
                pwd = entry.get_text()
                if pwd:
                    try:
                        if sudo_manager.validate_password(pwd):
                            sudo_manager.set_password(pwd)
                            self.user_password = pwd
                            self._begin_install(exe_path)
                        else:
                            self._show_auth_error()
                    except NameError:
                        self._begin_install(exe_path)

        dlg.connect("response", on_resp)
        entry.connect("activate", lambda _w: dlg.response("unlock"))
        try:
            translate_dialog(dlg)
        except NameError:
            pass
        dlg.present()

    def _show_auth_error(self):
        dlg = Adw.MessageDialog(
            heading=_("Authentication Failed"),
            body=_("Incorrect password."),
            transient_for=self.get_root() or self.window,
        )
        dlg.add_response("ok", _("OK"))
        try:
            translate_dialog(dlg)
        except NameError:
            pass
        dlg.present()

    # ── Installation flow ───────────────────────────────────────────

    def _begin_install(self, exe_path):
        self.install_started = True
        self.error_message = None
        self.progress_data = ""

        downloading = exe_path is None
        self.total_steps = 6 if downloading else 5

        self._set_state_installing()
        self._update_step(0, _("Preparing..."))

        threading.Thread(
            target=self._run_install, args=(exe_path, downloading), daemon=True
        ).start()

    def _update_step(self, step, label):
        self.step_label.set_label(label)
        self.progress_bar.set_fraction(step / self.total_steps)

    def _step_on_main(self, step, label):
        GLib.idle_add(self._update_step, step, label)

    def _run_install(self, exe_path, downloading):
        try:
            sudo_manager.start_privileged_session()
        except NameError:
            pass

        step = 0
        try:
            # ── Download (optional) ──
            if downloading:
                step += 1
                self._step_on_main(
                    step, _("Step {}: Downloading installer...").format(step)
                )
                tmp_path = os.path.join(tempfile.gettempdir(), "Affinity_x64.exe")
                total_steps = self.total_steps
                current_step = step

                def _reporthook(blocks, bsize, total):
                    if total > 0:
                        frac = min(blocks * bsize / total, 1.0)
                        GLib.idle_add(
                            self.progress_bar.set_fraction,
                            (current_step - 1 + frac) / total_steps,
                        )

                urllib.request.urlretrieve(
                    DOWNLOAD_URL, tmp_path, reporthook=_reporthook
                )
                exe_path = tmp_path

            # ── Install Wine ──
            step += 1
            self._step_on_main(
                step, _("Step {}: Installing Wine...").format(step)
            )

            try:
                env = sudo_manager.get_env()
                sudo_wrap = sudo_manager.wrapper_path
            except NameError:
                env = os.environ.copy()
                sudo_wrap = "sudo"

            self._run_cmd(
                f"{sudo_wrap} pacman -Sy wine-affinity --noconfirm && "
                f"chmod 775 $HOME/.local/share/linexin/scripts/wine-affinity-config.sh",
                env=env,
            )

            # ── Install required components ──
            step += 1
            self._step_on_main(
                step, _("Step {}: Installing required components...").format(step)
            )

            config_script = os.path.expanduser(
                "~/.local/share/linexin/scripts/wine-affinity-config.sh"
            )
            proc = subprocess.Popen(
                config_script, shell=True, stdin=subprocess.PIPE, text=True
            )
            proc.communicate(input="Y\n")
            if proc.returncode != 0:
                raise Exception(
                    _("Setting up the environment failed with exit code {}.").format(
                        proc.returncode
                    )
                )

            # ── Run Affinity installer ──
            step += 1
            self._step_on_main(
                step, _("Step {}: Installing Affinity...").format(step)
            )
            self._run_cmd(
                f"rum wine-affinity {shlex.quote(WINE_PREFIX)} wine {shlex.quote(exe_path)}"
            )

            # ── Create shortcuts ──
            step += 1
            self._step_on_main(
                step, _("Step {}: Creating application shortcuts...").format(step)
            )
            shortcuts_dir = os.path.expanduser("~/.local/share/linexin/shortcuts")
            apps_dir = os.path.expanduser("~/.local/share/applications")
            os.makedirs(apps_dir, exist_ok=True)

            for info in APP_REGISTRY.values():
                folder = os.path.join(AFFINITY_DIR, info["folder"])
                if os.path.isdir(folder):
                    src = os.path.join(shortcuts_dir, info["desktop"])
                    dst = os.path.join(apps_dir, info["desktop"])
                    if os.path.isfile(src):
                        shutil.move(src, dst)

            # ── Clean up ──
            step += 1
            self._step_on_main(
                step, _("Step {}: Cleaning up...").format(step)
            )

            desktop_dir = GLib.get_user_special_dir(
                GLib.UserDirectory.DIRECTORY_DESKTOP
            )
            if desktop_dir:
                lnk = os.path.join(desktop_dir, "Affinity.lnk")
                if os.path.exists(lnk):
                    try:
                        os.remove(lnk)
                    except OSError:
                        pass

            tmp_exe = os.path.join(tempfile.gettempdir(), "Affinity_x64.exe")
            if os.path.exists(tmp_exe):
                try:
                    os.remove(tmp_exe)
                except OSError:
                    pass

        except Exception as e:
            self.error_message = str(e)

        GLib.idle_add(self._finish_install)

    def _run_cmd(self, command, env=None):
        proc = subprocess.Popen(
            command,
            shell=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=env,
        )
        for line in iter(proc.stdout.readline, ""):
            if line:
                self.progress_data += line
        proc.stdout.close()
        rc = proc.wait()
        if rc != 0:
            raise subprocess.CalledProcessError(rc, command)

    def _finish_install(self):
        self.install_started = False
        try:
            sudo_manager.stop_privileged_session()
            sudo_manager.forget_password()
        except NameError:
            pass
        self.user_password = None

        if self.error_message:
            dlg = Adw.MessageDialog(
                heading=_("Installation failed"),
                body=self.error_message,
                transient_for=self.get_root() or self.window,
            )
            dlg.add_response("ok", _("OK"))
            try:
                translate_dialog(dlg)
            except NameError:
                pass
            dlg.present()
            self._set_state_pre_install()
        else:
            self._set_state_post_install()

        return False
