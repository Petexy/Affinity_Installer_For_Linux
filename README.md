# Affinity Installer for Linux 

## Supported distros:
- Linexin

## Distros supported after tinkering:
- Every Arch-based distro

## How to use (on Linexin):
The app is already pre-installed on your distro.

1. Download your desired Affinity suite program in .exe format
4. Run "Affinity Installer" from your Linexin Center
5. Press "Install Affinity application"
6. Install all components that will pop up on the screen (Wine Mono and .NET Framework two times)
7. Wait and select your downloaded Affinity installer .exe file
8. Run your program from the Application Launcher
<br><br><br>

## How to use (on Arch-Based):

1. Add linexin-repo to your pacman.conf file in /etc/pacman.conf and update repos by using pacman -Sy
2. Download your desired Affinity suite program in .exe format
3. Use pacman to install affinity-installer2 and linexin-center packages and all their dependencies
4. Run Linexin Center and Affinity Installer module
5. Press "Install Affinity application"
6. Install all components that will pop up on the screen 
7. Wait and select your downloaded Affinity installer .exe file
8. Run your program from the application launcher


## Troubleshooting:
- The app is fully written in Python. For some reason sometimes it fails to perform step 2 (executing wine-affinity-config.sh) if the "affinity-installer" is not in the /usr/bin directory. If that happens, you will need to move the script to the /usr/bin/ directory for it to work properly
- The app requires some dependencies: python-gobject, gtk4, libadwaita, python - Those are required to use the application. Be sure you have them installed if you use it on Arch-based other than Linexin.


