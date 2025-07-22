# Affinity Installer for Linux 

## Supported distros:
- Linexin

## Distros supported after tinkering:
- Every Arch-based distro

## How to use (on Linexin):
1. Download your desired Affinity suite program in .exe format
4. Run "Affinity Installer" from your Application Launcher
5. Press "Install Affinity application"
6. Install all components that will pop up on the screen (Wine Mono and .NET Framework two times)
7. Wait and select your downloaded Affinity installer .exe file
8. Run your program from the Application Launcher
<br><br><br>

## How to use (on Arch-Based):

There are two ways to properly use it on Arch and Arch-based distros other than Linexin.

### First one:
1. Download wine-affinity-config.sh script from [Linexin airootfs](https://github.com/Petexy/Linexin/blob/main/airootfs/etc/skel/.local/share/linexin/scripts/wine-affinity-config.sh))
2. Put it in the $HOME/.local/share/linexin/scripts/ and make it executable
3. Download desired app shortcut from [Linexin airootfs](https://github.com/Petexy/Linexin/tree/main/airootfs/etc/skel/.local/share/linexin/shortcuts)
4. Put it in the $HOME/.local/share/linexin/shortcuts/
5. Add linexin-repo to your pacman.conf file in /etc/pacman.conf and update repos by using pacman -Sy
6. Download your desired Affinity suite program in .exe format
7. Use pacman to install affinity-installer package and all it dependencies
9. Run program
10. Press "Install Affinity application"
11. Install all components that will pop up on the screen 
12. Wait and select your downloaded Affinity installer .exe file
13. Run your program from the application launcher

### Second one (if you don't want to add repo):

1. Download wine-affinity-config.sh script from [Linexin airootfs](https://github.com/Petexy/Linexin/blob/main/airootfs/etc/skel/.local/share/linexin/scripts/wine-affinity-config.sh))
2. Put it in the $HOME/.local/share/linexin/scripts/ and make it executable
3. Download desired app shortcut from [Linexin airootfs](https://github.com/Petexy/Linexin/tree/main/airootfs/etc/skel/.local/share/linexin/shortcuts)
4. Put it in the $HOME/.local/share/linexin/shortcuts/
5. Compile custom [Wine Build from ElementalWarrior](https://github.com/daniel080400/AffinityLinuxTut/tree/main?tab=readme-ov-file) by following the instruction from this Git
6. Copy compiled build to /opt/wines/wine-affinity
7. Download WinMetadata from Windows VM (C:\Windows\system32\WinMetadata)
8. Paste it in $HOME/.WineAffinity/drive_c/windows/system32/WinMetadata
9. Download your desired Affinity suite program in .exe format
10. Download affinity-installer
11. Make the affinity-installer file executable
12. Run program
13. Press "Install Affinity application"
14. Install all components that will pop up on the screen 
15. Wait and select your downloaded Affinity installer .exe file
16. Run your program from the application launcher

<br><br>

## Troubleshooting:
- The app is fully written in Python. For some reason sometimes it fails to perform step 2 (executing wine-affinity-config.sh) if the "affinity-installer" is not in the /usr/bin directory. If that happens, you will need to move the script to the /usr/bin/ directory for it to work properly
- The app requires some dependencies: python-gobject, gtk4, libadwaita, python - Those are required to use the application. Be sure you have them installed if you use it on Arch-based other than Linexin.


