# Everest Mod CLI

**WIP**: This project is under development. Expect breaking changes and limited functionality. Use at your own risk.

A command-line interface tool for managing Celeste mods using the maddie480's public online database.

This project currently targets **Linux** installation. The **Flatpak** version is not supported. **MacOS** might work, but it's not guaranteed

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [list](#list)
  - [show](#show)
  - [install](#install-mod_name)
  - [update](#update)
- [Motivation](#motivation)
- [Notes](#notes)
- [Acknowledgments](#acknowledgments)
- [License](#license)

## Features

- **Seamless Mod Management**: Easily manage mods directly from the terminal.
- **Install Mods by Name**: No need for Olympus or a web browser—just type the mod name to install.
- **Comprehensive Mod Listing**: View all installed mods along with their names and versions at a glance.
- **Installed Mod Details**: Easily check the dependencies of your installed mods for better management.
- **Automatic Update Checks**: Stay up-to-date with available updates for your installed mods, which can be installed automatically while you play—running in the background!
- **Asynchronous Downloads**: Experience reduced total download times, lower memory usage, and faster checksum verifications for a smoother experience. 

## Installation

Make sure you have installed [Rust](https://www.rust-lang.org/tools/install) and [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html).

Clone the repo and build it yourself using `cargo`.

```bash
git clone https://github.com/pinpinroku/everest-mod-cli.git
cd everest-mod-cli
cargo build --release
```

> If your CPU supports the **AVX2** feature, set the flag `RUSTFLAGS='-C target-feature=+avx2'` before building to accelerate hash calculation speed. You can check whether your CPU supports **AVX2** by running `lscpu | grep avx2`.

Then symlink the binary to the local bin directory.

```bash
mkdir -p ~/.local/bin/
ln -s $HOME/everest-mod-cli/target/release/everest-mod-cli $HOME/.local/bin/everest-mod-cli
```

> We plan to release the built binary files once the specifications are finalized (stabilized).

## Usage

```bash
everest-mod-cli [OPTIONS] [COMMAND] 
```

Available commands:

### `list`

List all installed mods, showing their actual names and versions.
```bash
everest-mod-cli list
# Collecting information about installed mods... This might take a few minutes if your mods library is huge
#
# Installed mods (138 found):
# - AdamsAddons (version 1.13.3)
# - AdventureHelper (version 1.6.0)
# - AidenHelper (version 1.2.1)
# - AltSidesHelper (version 1.7.1)
# - Anonhelper (version 1.1.1)
# - ArphimigonsToyBox (version 1.4.0)
# - AurorasHelper (version 0.12.2)
# - AvBdayHelper2021 (version 1.0.3)
# - BGswitch (version 1.2.2)
# - Batteries (version 1.1.4)
# ...
```

### `show <mod_name>`

Show the details of a specific mod that have been installed.
```bash
everest-mod-cli show "Iceline_silentriver"
# Checking installed mod information...
# Mod Information:
# - Name: Iceline_silentriver
# - Version: 1.1
#
# Dependencies:
#  - Everest v1.4.0.0
#  - SkinModHelper v0.6.1
#  - IcelineLoadingAnim v1.0.0
```

### `install <mod_name>`

Install a mod by its name. The mod will be downloaded and installed in the appropriate directory.
Checksum verification is performed automatically to ensure the integrity of the downloaded mod.
```bash
everest-mod-cli install "SpeedrunTool"
# Starting installation of the mod 'SpeedrunTool'...
# Downloading mod files...
#   [00:00:08] [################################################] 245.41 KiB/245.41 KiB (0s)
# Verifying checksum...
# Checksum verified
# Installation finished successfully!
```

### `update`

Check for available updates for installed mods.
```bash
# Check for updates
everest-mod-cli update
# Checking mod updates...
# Available updates:
# 
# StrawberryJam2021
#  - Current version: 1.0.11
#  - Available version: 1.0.12
# 
# Run with --install to install these updates
```

Install available updates.
```bash
# Check and install available updates
everest-mod-cli update --install
# Checking mod updates...
# Available updates:
# 
# StrawberryJam2021
#  - Current version: 1.0.11
#  - Available version: 1.0.12
# 
# Installing updates...
# 
# Updating StrawberryJam2021...
#   [00:03:26] [################################################] 91.22 MiB/91.22 MiB (0s)
#   Verifying checksum...
#   Checksum verified!
#   Updated StrawberryJam2021 to version 1.0.12
# 
# All updates installed successfully!
```

## Option

You can specify your custom mods directory using `--mods-dir`.
```bash
# Install the mod "SpeedrunTool" while specifying the mods directory
everest-mod-cli --mods-dir /home/maddy/game/exokgames/celeste/Mods/ install "SpeedrunTool"
```
> The directory should have permissions of at least 0700.

## Motivation

Everest and Olympus are excellent tools for managing Celeste mods. However, there are still some quality-of-life improvements that could be made:

- Olympus is unstable or completely non-functional on certain Linux distributions, particularly in **Wayland** environments.
- Download speed is slow in some countries.
  - CLI tools like `curl` or `wget` are sometimes faster than in-game downloads.
  - Cannot run auto updates on background.
  - Need to wait or entirely cancel the updates when opening the game.
- No option to *cancel*, *pause*, or *resume* downloads in mod menu.
- Lack of clarity about `dependencies` when uninstalling mods.

## Notes

- The `mod_name` and the corresponding filenames may not match.
- The `mod_name` refers to the name of the Mod as it appears in the game menu.
- The `filename` is the name of the zip file that contains the Mod's assets and the manifest file called `everest.yaml`.

## Acknowledgments

This project was made possible thanks to the [EverestUpdateCheckerServer](https://github.com/maddie480/EverestUpdateCheckerServer) hosted by [maddie480](https://github.com/maddie480). We're grateful for their work, which has been instrumental in the development of this project.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
