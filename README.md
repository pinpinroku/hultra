# Everest Mod CLI

A command line tool to help manage mods for the 2D platformer Celeste.

This project currently targets **Linux** installation. **macOS** might work, but it's not guaranteed.

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [list](#list)
  - [show](#show)
  - [install](#install)
  - [update](#update)
- [Motivation](#motivation)
- [Notes](#notes)
- [Bug Reports](#bug-reports)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)
- [Contact](#contact)

---

## Features

- **Seamless Mod Management**: Easily manage mods directly from the terminal.
- **Comprehensive Mod Listing**: View all installed mods along with their names and versions at a glance.
- **Installed Mod Details**: Easily check the dependencies of your installed mods for better management.
- **Automatic Update Checks**: Stay up-to-date with available updates for your installed mods, which can be installed automatically while you play—running in the background!
- **Install Mods by URL**: Just type the URL of the mod page to install the mod. All missing dependencies are resolved automatically.
- **Asynchronous Downloads**: Experience reduced total download times, lower memory usage, and faster checksum verifications for a smoother experience. 

---

## Installation

Just download the binary from the [release](https://github.com/pinpinroku/everest-mod-cli/releases) page.

Once downloaded, give it execution permissions by running `chmod u+x everest-mod-cli`. Then, move it to `~/.local/bin` (or another directory of your choice). Finally, add that directory to your PATH if it isn’t already included.

### Build yourself

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/pinpinroku/everest-mod-cli.git
   cd everest-mod-cli
   ```

2. **Build the Project** (requires [Rust](https://www.rust-lang.org/) installed):
   ```bash
   cargo build --release
   ```

3. **Run the CLI**:
   ```bash
   ./target/release/everest-mod-cli
   ```
---

## Usage

- Show Help
```bash
everest-mod-cli --help
```

- Run a Command:
```bash
everest-mod-cli [OPTIONS] [COMMAND] 
```

**Available commands**:

### `list`

List all installed mods, showing their actual names and versions.
```bash
everest-mod-cli list
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

### `show`

`everest-mod-cli show [mod_name]`

Show the details of a specific mod that have been installed.
```bash
everest-mod-cli show "zbs_Crystal"
# Checking installed mod information...
#
# Mod Information:
# - Name: zbs_Crystal
# - Version: 1.2.8
#   Dependencies:
#   - Name: Everest
#   - Version: 1.3971.0
#   Optional Dependencies:
#   - Name: SaladimHelper
```

### `install`

`everest-mod-cli install [page_url]`

Install a mod by the URL of the page where the mod is featured on.

Checksum verification is performed automatically to ensure the integrity of the downloaded mod.

If there are missing dependencies, it will automatically download and install them.
```bash
everest-mod-cli install "https://gamebanana.com/mods/592695"
# 🌐 Fetching online database...
# 🍓 kit                5.55 MiB  21.89 KiB/s  00:00:00 [#######################]  100%
# 🍓 CommunalHelper    19.25 MiB   4.40 MiB/s  00:00:00 [#######################]  100%
# 🍓 GravityHelper    706.45 KiB  97.16 KiB/s  00:00:00 [#######################]  100%
# 🍓 OutbackHelper     45.12 KiB  61.97 KiB/s  00:00:00 [#######################]  100%
# 🍓 VivHelper          6.24 MiB   2.54 MiB/s  00:00:00 [#######################]  100%
# 🍓 AdventureHelper   86.87 KiB  86.80 KiB/s  00:00:00 [#######################]  100%
# 🍓 MaxHelpingHand   987.20 KiB 594.86 KiB/s  00:00:00 [#######################]  100%
# 🍓 ShroomHelper       1.74 MiB   1.15 MiB/s  00:00:00 [#######################]  100%
# 🍓 VortexHelper       2.09 MiB   1.36 MiB/s  00:00:00 [#######################]  100%
# 🍓 XaphanHelper       6.62 MiB   5.01 MiB/s  00:00:00 [#######################]  100%
# 🍓 DJMapHelper      422.93 KiB 629.41 KiB/s  00:00:00 [#######################]  100%
# All required dependencies installed successfully!
```

> Attached berry indicates download completed.

### `update`

Check for available updates for installed mods.
```bash
everest-mod-cli update
# 🌐 Fetching online database...
#
# UnderDragon's Repository: 2.5.3 -> 2.5.4
# califonia dreamin': 0.0.1 -> 0.0.1
#
# Run with --install to install these updates
```

Install available updates.
```bash
everest-mod-cli update --install
# 🌐 Fetching online database...
# 
# UnderDragon's Repository: 2.5.3 -> 2.5.4
# califonia dreamin': 0.0.1 -> 0.0.1
# 
# Installing updates...
# 🍓 califonia dreamin'          6.91 MiB 254.93 KiB/s  00:00:00 [######################]  100%
# 🍓 UnderDragon's Repository   29.70 MiB 683.38 KiB/s  00:00:00 [######################]  100%
```

> Modders sometimes forget to increase the version number but the file change will be detected by the checksum.

## Options

### `-d, --mods-dir` \<DIR\>

The default mods directory is set to the Steam game installation folder:

`~/.local/share/Steam/steamapps/common/Celeste/Mods/`

You can specify your custom mods directory using `--mods-dir`.
```bash
# Install the mod "SpeedrunTool" while specifying the mods directory
everest-mod-cli --mods-dir /home/maddy/game/exokgames/celeste/Mods/ install "SpeedrunTool"
```
> The directory should have permissions of at least 0700.

Just use an alias to make things easier:

```bash
#!/usr/bin/env bash
# ~/.bashrc
alias emc='everest-mod-cli --mods-dir $HOME/game/exokgames/celeste/Mods/'
```

### `-m, --mirror-priority` \<MIRROR\>

> This option only applies to the `install` and the `update` commands.

Mirror priority can be specified by a comma-separated list. Default is "otobot,gb,jade,wegfan".

| name    | location                      |
|---------|-------------------------------|
| gb      | Default GameBanana Server     |
| jade    | Germany                       |
| wegfan  | China                         |
| otobot  | North America                 |

If the download from the current server fails, the application will automatically fall back to the next server in the priority list to retry the download.

You can also restrict the fallback servers by providing a comma-separated list (e.g., \"otobot,jade\"), which will limit the retries to only those specified servers.

---

## Motivation

Everest and Olympus are excellent tools for managing Celeste mods. However, there are still some quality-of-life improvements that could be made:

- Olympus is unstable or completely non-functional on certain Linux distributions, particularly in **Wayland** environments.
- Download speed is slow in some countries.
  - CLI tools like `curl` or `wget` are sometimes faster than in-game downloads.
  - Cannot run auto updates on background.
  - Need to wait or entirely cancel the updates when opening the game.
- No option to *cancel*, *pause*, or *resume* downloads in the in-game mod menu to resolve missing dependencies.
- Lack of clarity about `dependencies` when uninstalling mods.

---

## Notes

- The `mod_name` and the corresponding filenames may not match.
- The `mod_name` is the unique identifier which is stored in the metadata and online database for searching purpose.
- The `filename` is the name of the zip file that contains the Mod's assets and the manifest file called `everest.yaml`.

---

## Bug Reports

If you encounter any issues or bugs, please open an issue using the bug report template:

[Submit Bug Report](https://github.com/pinpinroku/everest-mod-cli/issues/new?assignees=&labels=&template=bug_report.md&title=)

---

## Contributing

We welcome contributions to Everest Mod CLI! To contribute:

1. Fork the repository.
2. Create a new branch for your feature or bugfix.
3. Submit a pull request with a clear explanation of your changes.

For more details, check the [Contribution Guidelines](https://github.com/pinpinroku/everest-mod-cli/blob/main/CONTRIBUTING.md).

---

## License

This project is licensed under the [MIT License](https://github.com/pinpinroku/everest-mod-cli/blob/main/LICENSE).

---

## Acknowledgments

This project was made possible thanks to the [EverestUpdateCheckerServer](https://github.com/maddie480/EverestUpdateCheckerServer) hosted by [maddie480](https://github.com/maddie480). We're grateful for their work, which has been instrumental in the development of this project.

---

## Contact

For questions or support, reach out via email or open a discussion in the [Discussions tab](https://github.com/pinpinroku/everest-mod-cli/discussions/new?category=q-a).
