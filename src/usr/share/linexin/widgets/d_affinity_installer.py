#!/usr/bin/env python3

import gi
import subprocess
import threading
import gettext
import locale
import os
import shutil

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GLib

# --- Localization Setup ---
APP_NAME = "affinity-installer"
LOCALE_DIR = "/usr/share/locale" 

# Set up the locale environment
try:
    locale.setlocale(locale.LC_ALL, '')
    lang, _ = locale.getlocale()
    if lang is None:
        lang = 'en'
except locale.Error:
    lang = 'en'

gettext.bindtextdomain(APP_NAME, LOCALE_DIR)
gettext.textdomain(APP_NAME)
translation = gettext.translation(APP_NAME, LOCALE_DIR, languages=[lang], fallback=True)
translation.install()
_ = gettext.gettext

class AffinityInstallerWidget(Gtk.Box):
    def __init__(self, hide_sidebar=False, window=None):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        
        # Required: Widget display name
        self.widgetname = _("Affinity Installer")
        
        # Optional: Widget icon
        self.widgeticon = "/usr/share/icons/github.petexy.affinityinstaller.png"
        
        # Widget content
        self.set_margin_top(12)
        self.set_margin_bottom(50)
        self.set_margin_start(12)
        self.set_margin_end(12)
        
        # Initialize state variables
        self.progress_visible = False
        self.progress_data = ""
        self.install_started = False
        self.error_message = None
        self.current_product = "Affinity"
        
        # Create main content stack
        self.content_stack = Gtk.Stack()
        self.content_stack.set_transition_type(Gtk.StackTransitionType.SLIDE_UP_DOWN)
        self.content_stack.set_hexpand(True)
        self.content_stack.set_vexpand(True)
        self.append(self.content_stack)
        
        # Setup different views
        self.setup_welcome_view()
        self.setup_info_view()
        self.setup_progress_view()
        
        # Controls section
        self.setup_controls()
        
        # Set initial view
        self.content_stack.set_visible_child_name("welcome_view")

        self.window = window
        self.hide_sidebar = hide_sidebar

        if not self.hide_sidebar:
            pass
        else:
            # Single widget mode
            GLib.idle_add(self.resize_window_deferred)
    
    def resize_window_deferred(self):
        """Called after widget initialization to resize window safely"""
        if self.window:
            try:
                self.window.set_default_size(800, 600)
                print("Window default size set to 1400x800")
            except Exception as e:
                print(f"Failed to resize window: {e}")
        return False


    def setup_welcome_view(self):
        """Setup the welcome view with icon and description"""
        welcome_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=15)
        welcome_box.set_valign(Gtk.Align.CENTER)
        welcome_box.set_halign(Gtk.Align.CENTER)
        
        # Welcome icon
        welcome_image = Gtk.Image()
        if os.path.exists("/usr/share/icons/github.petexy.affinityinstaller.png"):
            welcome_image.set_from_file("/usr/share/icons/github.petexy.affinityinstaller.png")
        else:
            welcome_image.set_from_icon_name("package-x-generic")
        welcome_image.set_pixel_size(64)
        welcome_box.append(welcome_image)
        
        # Title
        title = Gtk.Label(label=_("Affinity Installer"))
        title.add_css_class("title-2")
        welcome_box.append(title)
        
        # Description
        description = Gtk.Label()
        description.set_markup(_("Install the Affinity suite (V2) for Linux\n\nSupports Affinity Photo 2, Designer 2, and Publisher 2"))
        description.set_justify(Gtk.Justification.CENTER)
        description.add_css_class("dim-label")
        welcome_box.append(description)
        
        self.content_stack.add_named(welcome_box, "welcome_view")

    def setup_info_view(self):
        """Setup the info view for status messages"""
        info_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=15)
        info_box.set_valign(Gtk.Align.CENTER)
        info_box.set_halign(Gtk.Align.CENTER)
        
        # Status images
        self.fail_image = Gtk.Image()
        self.fail_image.set_from_icon_name("dialog-error")
        self.fail_image.set_pixel_size(48)
        self.fail_image.set_visible(False)
        
        self.success_image = Gtk.Image()
        self.success_image.set_from_icon_name("emblem-ok")
        self.success_image.set_pixel_size(48)
        self.success_image.set_visible(False)
        
        info_box.append(self.fail_image)
        info_box.append(self.success_image)
        
        # Status label
        self.info_label = Gtk.Label()
        self.info_label.set_wrap(True)
        self.info_label.set_justify(Gtk.Justification.CENTER)
        info_box.append(self.info_label)
        
        self.content_stack.add_named(info_box, "info_view")

    def setup_progress_view(self):
        """Setup the progress view with terminal output"""
        self.output_buffer = Gtk.TextBuffer()
        self.output_textview = Gtk.TextView.new_with_buffer(self.output_buffer)
        self.output_textview.set_wrap_mode(Gtk.WrapMode.WORD)
        self.output_textview.set_editable(False)
        self.output_textview.set_cursor_visible(False)
        self.output_textview.set_monospace(True)
        self.output_textview.set_left_margin(10)
        self.output_textview.set_right_margin(10)
        self.output_textview.set_top_margin(5)
        self.output_textview.set_bottom_margin(5)
        
        scrolled_window = Gtk.ScrolledWindow()
        scrolled_window.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        scrolled_window.set_child(self.output_textview)
        scrolled_window.set_min_content_height(200)
        
        output_frame = Gtk.Frame()
        output_frame.set_child(scrolled_window)
        self.content_stack.add_named(output_frame, "progress_view")

    def setup_controls(self):
        """Setup control buttons"""
        controls_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        controls_box.set_halign(Gtk.Align.CENTER)
        
        self.btn_install = Gtk.Button(label=_("Select Installer File"))
        self.btn_install.add_css_class("suggested-action")
        self.btn_install.add_css_class("buttons_all")
        self.btn_install.connect("clicked", self.on_install_clicked)
        
        self.btn_toggle_progress = Gtk.Button(label=_("Show Progress"))
        self.btn_toggle_progress.set_sensitive(False)
        self.btn_toggle_progress.set_visible(False)
        self.btn_toggle_progress.add_css_class("buttons_all")
        self.btn_toggle_progress.connect("clicked", self.on_toggle_progress_clicked)
        
        controls_box.append(self.btn_install)
        controls_box.append(self.btn_toggle_progress)
        
        self.append(controls_box)

    def on_install_clicked(self, button):
        """Handle install button click - opens file chooser"""
        self.create_file_chooser()

    def create_file_chooser(self):
        """Creates and shows a file chooser dialog"""
        title = _("Select Affinity .exe file")
        
        # Get the toplevel window to use as parent
        parent_window = self.get_root()
        
        file_chooser = Gtk.FileChooserDialog(
            title=title,
            transient_for=parent_window,
            modal=True,
            action=Gtk.FileChooserAction.OPEN
        )
        
        file_chooser.add_button(_("Cancel"), Gtk.ResponseType.CANCEL)
        file_chooser.add_button(_("Open"), Gtk.ResponseType.OK)
        
        file_filter = Gtk.FileFilter()
        file_filter.set_name(_("Affinity Installer (*.exe)"))
        file_filter.add_pattern("*.exe")
        file_chooser.add_filter(file_filter)
        
        file_chooser.connect("response", self.on_file_chooser_response)
        file_chooser.present()

    def on_file_chooser_response(self, dialog, response_id):
        """Handles file selection and starts the installation"""
        if response_id == Gtk.ResponseType.OK:
            file = dialog.get_file()
            if file:
                exe_file_path = file.get_path()
                self.begin_install(exe_file_path)
            else:
                self.show_error_message(_("No file selected. Installation cancelled."))
        else:
            self.show_error_message(_("Installation cancelled."))
        
        dialog.destroy()

    def show_error_message(self, message):
        """Display error message"""
        self.info_label.set_markup(f'<span color="#e01b24" weight="bold">{message}</span>')
        self.fail_image.set_visible(True)
        self.success_image.set_visible(False)
        self.content_stack.set_visible_child_name("info_view")

    def begin_install(self, exe_path):
        """Start the installation process"""
        self.install_started = True
        self.btn_install.set_sensitive(False)
        self.btn_install.set_visible(False)
        self.btn_toggle_progress.set_sensitive(True)
        self.btn_toggle_progress.set_visible(True)
        self.error_message = None

        # Reset images
        self.fail_image.set_visible(False)
        self.success_image.set_visible(False)

        install_message = _("Installing {}... This may take a while...").format(self.current_product)
        self.info_label.set_markup(f'<span size="large" weight="bold">{install_message}</span>')
        self.content_stack.set_visible_child_name("info_view")
        
        self.progress_data = ""
        self.progress_visible = False
        self.btn_toggle_progress.set_label(_("Show Progress"))
        self.output_buffer.set_text("")

        self.run_installation_flow(exe_path)

    def on_toggle_progress_clicked(self, button):
        """Handle progress toggle button"""
        self.progress_visible = not self.progress_visible

        if self.progress_visible:
            self.btn_toggle_progress.set_label(_("Hide Progress"))
            self.output_buffer.set_text(self.progress_data or _("[console output]"))
            self.content_stack.set_visible_child_name("progress_view")
            GLib.timeout_add(50, self.scroll_to_end)
        else:
            self.btn_toggle_progress.set_label(_("Show Progress"))
            self.content_stack.set_visible_child_name("info_view")

    def run_installation_flow(self, exe_path):
        """Execute the complete installation flow"""
        def stream_output():
            try:
                # Helper to run a non-interactive command and stream its output for the log
                def run_and_log(command, fail_msg):
                    process = subprocess.Popen(
                        command, shell=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                        text=True, encoding='utf-8', errors='replace'
                    )
                    for line in iter(process.stdout.readline, ''):
                        if line:
                            self.progress_data += line
                            GLib.idle_add(self.update_output_buffer, self.progress_data)
                    process.stdout.close()
                    return_code = process.wait()
                    if return_code != 0:
                        raise subprocess.CalledProcessError(return_code, command, output=fail_msg)
                    return True

                # Step 1: Pacman Command
                self.progress_data += _("Step 1: Updating and installing wine-affinity...\n\n")
                GLib.idle_add(self.update_output_buffer, self.progress_data)
                pacman_cmd = "run0 pacman -Sy wine-affinity --noconfirm && chmod 775 $HOME/.local/share/linexin/scripts/wine-affinity-config.sh"
                run_and_log(pacman_cmd, _("Failed to install wine-affinity."))

                # Step 2: Wine Config Script
                self.progress_data += _("\nStep 2: Configuring Wine environment...\n")
                self.progress_data += _("This step will run 'winetricks' and may open new windows. Please follow any on-screen instructions.\n")
                self.progress_data += _("The installer will continue automatically after this step is complete.\n\n")
                GLib.idle_add(self.update_output_buffer, self.progress_data)

                config_script_path = os.path.expanduser("~/.local/share/linexin/scripts/wine-affinity-config.sh")
                config_process = subprocess.Popen(
                    config_script_path, shell=True, stdin=subprocess.PIPE, text=True
                )
                config_process.communicate(input='Y\n')

                if config_process.returncode != 0:
                    raise Exception(_("The Wine configuration script failed with exit code {}.").format(config_process.returncode))

                self.progress_data += _("Wine configuration complete.\n")
                GLib.idle_add(self.update_output_buffer, self.progress_data)
                
                # Step 3: Run Affinity Installer via Wine
                self.progress_data += _("\nStep 3: Running the Affinity installer. Please follow the on-screen instructions...\n\n")
                self.progress_data += _("Remember to install .NET if the installer prompts you.\n\n")
                GLib.idle_add(self.update_output_buffer, self.progress_data)
                wine_cmd = f'rum wine-affinity "{os.path.expanduser("~/.WineAffinity")}" wine "{exe_path}"'
                run_and_log(wine_cmd, _("The Affinity installer did not complete successfully."))

                # Step 4: Move Desktop Files
                self.progress_data += _("\nStep 4: Creating application shortcuts...\n\n")
                GLib.idle_add(self.update_output_buffer, self.progress_data)
                
                affinity_dir = os.path.expanduser("~/.WineAffinity/drive_c/Program Files/Affinity")
                shortcuts_dir = os.path.expanduser("~/.local/share/linexin/shortcuts")
                applications_dir = os.path.expanduser("~/.local/share/applications")
                folder_desktop_map = {
                    "Photo 2": "photo-2.desktop",
                    "Designer 2": "designer-2.desktop",
                    "Publisher 2": "publisher-2.desktop"
                }

                os.makedirs(applications_dir, exist_ok=True)
                apps_found = False
                for folder, desktop_file in folder_desktop_map.items():
                    if os.path.isdir(os.path.join(affinity_dir, folder)):
                        apps_found = True
                        source_path = os.path.join(shortcuts_dir, desktop_file)
                        dest_path = os.path.join(applications_dir, desktop_file)
                        if os.path.isfile(source_path):
                            shutil.move(source_path, dest_path)
                            log_msg = _("Moved {} to {}\n").format(desktop_file, applications_dir)
                            self.progress_data += log_msg
                            GLib.idle_add(self.update_output_buffer, self.progress_data)
                        else:
                            log_msg = _("Warning: Desktop shortcut {} not found in {}\n").format(desktop_file, shortcuts_dir)
                            self.progress_data += log_msg
                            GLib.idle_add(self.update_output_buffer, self.progress_data)
                
                if not apps_found:
                    self.progress_data += _("Warning: No Affinity application folders found to create shortcuts.\n")
                    GLib.idle_add(self.update_output_buffer, self.progress_data)

            except Exception as e:
                self.error_message = str(e)
                self.progress_data += _("\nError: {}").format(e)
                GLib.idle_add(self.update_output_buffer, self.progress_data)

            GLib.idle_add(self.finish_installation)

        threading.Thread(target=stream_output, daemon=True).start()

    def update_output_buffer(self, text):
        """Update the output buffer with new text"""
        if self.progress_visible:
            self.output_buffer.set_text(text)
            GLib.idle_add(self.scroll_to_end)
        return False

    def scroll_to_end(self):
        """Scroll text view to the end"""
        end_iter = self.output_buffer.get_end_iter()
        self.output_textview.scroll_to_iter(end_iter, 0.0, False, 0.0, 0.0)
        return False

    def finish_installation(self):
        """Handle installation completion"""
        self.install_started = False
        self.btn_install.set_sensitive(True)
        self.btn_install.set_visible(True)
        self.btn_toggle_progress.set_sensitive(False)
        self.btn_toggle_progress.set_visible(False)
        
        if self.error_message:
            self.info_label.set_markup(f'<span color="#e01b24" weight="bold" size="large">{_("Installation failed: ")}</span>\n{self.error_message}')
            self.fail_image.set_visible(True)
        else:
            self.info_label.set_markup(f'<span color="#2ec27e" weight="bold" size="large">{_("Successfully installed {}!").format(self.current_product)}</span>')
            self.success_image.set_visible(True)
        
        self.content_stack.set_visible_child_name("info_view")
        self.progress_visible = False
        self.btn_toggle_progress.set_label(_("Show Progress"))
        
        return False