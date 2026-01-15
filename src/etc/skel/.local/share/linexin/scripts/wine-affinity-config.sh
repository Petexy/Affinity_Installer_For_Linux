#!/usr/bin/env bash

rum wine-affinity "$HOME/.WineAffinity" winetricks dotnet48 corefonts
mkdir -p "$HOME/.WineAffinity/drive_c/windows/system32/WinMetadata"
git clone https://github.com/daniel080400/AffinityLinuxTut
unzip AffinityLinuxTut/WinMetadata.zip
rum wine-affinity $HOME"/.WineAffinity" wine winecfg -v win11
cp -rf WinMetadata "$HOME/.WineAffinity/drive_c/windows/system32"
rum wine-affinity "$HOME/.WineAffinity" winetricks renderer=vulkan
rm -rf AffinityLinuxTut
