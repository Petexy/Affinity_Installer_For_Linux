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


## How to use (on Arch-Based):
1. Download wine-affinity-config.sh script from [Linexin airootfs](https://github.com/Petexy/Linexin/blob/main/airootfs/etc/skel/.local/share/linexin/scripts/wine-affinity-config.sh))
2. Put it in the $HOME/.local/share/linexin/scripts/
3. Make it executable
4. Download desired app shortcut from [Linexin airootfs](https://github.com/Petexy/Linexin/tree/main/airootfs/etc/skel/.local/share/linexin/shortcuts)
5. Put it in the $HOME/.local/share/linexin/shortcuts/
6. Compile custom [Wine Build from ElementalWarrior](https://github.com/daniel080400/AffinityLinuxTut/tree/main?tab=readme-ov-file) by following the instruction from this Git
7. Copy compiled build to /opt/wines/wine-affinity
8. Download WinMetadata from Windows VM (C:\Windows\system32\WinMetadata)
9. Paste it in $HOME/.WineAffinity/drive_c/windows/system32/WinMetadata
10. Download your desired Affinity suite program in .exe format
11. Download affinity-installer
12. Make the affinity-installer file executable
13. Run program
14. Press "Install Affinity application"
15. Install all components that will pop up on the screen 
16. Wait and select your downloaded Affinity installer .exe file
17. Run your program from the application launcher
