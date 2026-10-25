# hultra

A command-line installer and updater for Celeste mods on Linux.

---

## Table of Contents

- [Features](#features)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Usage](#usage)
- [Shell Completions](#shell-completions)
- [Global Options](#global-options)
- [Download Options](#download-options)
- [Technical Details](#technical-details)
- [Motivation](#motivation)
- [Bug Reports](#bug-reports)
- [License](#license)
- [Acknowledgments](#acknowledgments)
- [Contact](#contact)

---

## Features

- **Seamless Mod Management**: Easily manage mods directly from the terminal.
- **Comprehensive Mod Listing**: View all installed mods along with their names and versions at a glance.
- **Blazingly Fast Update**: Update your installed mods more faster than in-game update.
- **Install Mods by URL**: Just type the URL of the mod page to install the mod. All missing dependencies are resolved automatically.
- **Asynchronous Downloads**: Experience reduced total download times, lower memory usage, and faster checksum verifications for a smoother experience.

---

## Prerequisites

- Linux is running
- Celeste is installed on Steam
- Everest is installed

---

## Installation

Just download the binary from the [release](https://github.com/pinpinroku/hultra/releases) page.

Once downloaded, give it execution permissions by running `chmod u+x hultra`. Then, move it to `~/.local/bin` (or another directory of your choice). Finally, add that directory to your PATH if it isn't already included.

> The pre-build binary might not work for some users, then try another methods described below.

### Using cargo

If you have rust installed on your machine, just run following command:
```bash
cargo install --locked --git https://github.com/pinpinroku/hultra
```

This will automatically install the binary in ~/.cargo/bin, so you can use it immediately without hussle.

### Build yourself

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/pinpinroku/hultra.git
   cd hultra
   ```

2. **Build the Project** (requires [Rust](https://www.rust-lang.org/) installed):
   ```bash
   cargo build --release
   ```

3. **Run the CLI**:
   ```bash
   ./target/release/hultra
   ```
---

## Usage

- Show Help
```bash
hultra --help
```

- Run a Command
```bash
hultra [global_opts] [command] [options] [\<args...\>]
```

- List installed mods
```bash
hultra list
```
> The file name and mod name may differ. If so, it will be displayed in an informative way.

- Update installed mods
```bash
hultra update
```

- Install mods
```bash
# usage
hultra install URL [URL...]

# install a mod
hultra install https://gamebanana.com/mods/download/150453#FileInfo_1247390

# install multiple mods at once
hultra install https://gamebanana.com/mods/download/150453#FileInfo_1247390 https://gamebanana.com/mods/download/641987#FileInfo_1607057
```

Get the mod file URL in GameBanana:

1. Open the page of any mod you want to download in GameBanana.
2. In the Files section below the description, copy the link behind the `Manual Download` button.
3. Alternatively, click `Manual Download` and copy the URL of the page it takes you to.

You can also use various download URLs of GameBanana mirrors.

Allowed patterns:
- `https://celestemodupdater.0x0a.de/banana-mirror/\<id\>.zip`
- `https://celeste.weg.fan/api/v2/download/gamebanana-files/\<id\>`
- `https://banana-mirror-mods.celestemods.com/\<id\>.zip`
- `https://library.risingsunlight.dev/celeste/mods/\<id\>.zip`
- `https://gamebanana.com/mmdl/\<id\>`
- `https://gamebanana.com/mods/download/\<gbid\>#FileInfo_\<id\>`

You can get mirror URLs in [Banana Mirror Browser](https://maddie480.ovh/celeste/banana-mirror-browser) maintained by `maddie480`. Click the `Download` button, then in the popup that appears, right-click the `Download` button and copy the link.

> You can also specify a mix of multiple mirror URLs (domains). However, downloads will be prioritized according to the mirror specified by the `--mirror-priority` option. Regardless of which URL is used, all URLs are converted to the priority 1 URL before the download is executed.

## Shell Completions

You can generate shell completion scripts for bash, zsh, and fish using the `generate-completion` subcommand.

### Generate completions
```bash
# For Bash
$ hultra generate-completion bash > hultra.bash

# For Zsh
$ hultra generate-completion zsh > _hultra

# For Fish
$ hultra generate-completion fish > hultra.fish
```

### Installation
To enable completions, move the generated file to the appropriate directory or source it in your shell configuration.

**Bash**
```bash
$ hultra generate-completion bash | sudo tee /usr/share/bash-completion/completions/hultra
```

**Zsh**
```bash
# Create a directory for completions if it doesn't exist
$ mkdir -p ~/.zfunc
$ my_app completion zsh > ~/.zfunc/_my_app

# Add the following to your .zshrc:
fpath+=~/.zfunc
autoload -U compinit && compinit
```

**Fish**
```fish
$ hultra generate-completion fish > ~/.config/fish/completions/hultra.fish
```

## Global Options

Options can be applied globally.

### `-d, --directory` \<DIR\>

The default game directory is set to the Steam game installation folder:

`~/.local/share/Steam/steamapps/common/Celeste/`

You can specify your custom game directory using `--directory`.
```bash
# Install the mod "SpeedrunTool" while specifying the game directory
hultra --directory ~/game/exokgames/celeste/ install "SpeedrunTool"
```
> The directory should have permissions of at least 0700.

## Download Options

Options can be used for commands: `install` and `update`.

### `-p, --mirror-priority` \<MIRROR\>

Mirror priority can be specified by a comma-separated list. Default is "otobot,gb,jade,wegfan".

| name    | location                      |
|---------|-------------------------------|
| gb      | Default GameBanana Server     |
| jade    | Germany                       |
| wegfan  | China                         |
| otobot  | North America                 |

If the download from the current server fails, the application will automatically fall back to the next server in the priority list to retry the download.

You can also restrict the fallback servers by providing a comma-separated list (e.g., \"otobot,jade\"), which will limit the retries to only those specified servers.

**Example**
```bash
hultra update -p jade,wegfan update
```

### `-m, --use-api-mirror`

Enable this option to fetch the database from a GitHub Pages mirror. This may result in substantially faster processing time for installation and updates, especially for users experiencing connectivity issues with the primary source.

### `-j, --jobs` \<NUM\>

Limit concurrent downloads by specifying number from 1 to 6. Default to 4.

> Caution: See [Technical Details](#technical-details) section below about RAM usage.
---

## Technical Details

**Memory Usage & Privacy**

By default, this tool buffers downloads in memory to ensure:
1. **Performance:** Avoiding slow I/O overhead on filesystems, especially when `tmpfs` and `zram` are available.
2. **Security/Privacy:** Minimizing the trace of sensitive data on physical storage.

In the worst-case scenario (specific file combinations; especially audio files), memory usage can peak at ~2.2 GiB. If you want to keep memory usage low, please use the `--jobs 1` (or `-j 1`) flag to reduce the memory footprint.

### High-Impact Files (Examples)

The following mods contain large assets that significantly increase memory pressure:

- **Breeze Contest (Audio):** ~707 MB
- **The Celeste Parable:** ~523 MB
- **Spring Collab 2020 (Audio):** ~513 MB
- **Secret Santa Collab 2023 (Audio):** ~489 MB
- **Strawberry Jam (Audio):** 3 files, ~300 MB each
- **Secret Santa Collab 2024 (Audio):** 3 files, 450 MB + 350 MB (x2)

---

## Motivation

Everest and Olympus are excellent tools for managing Celeste mods. However, there are still some quality-of-life improvements that could be made:

- Olympus is unstable or completely non-functional on certain Linux distributions, particularly in **Wayland** environments.
- Download speed is slow in some regions.
  - CLI tools like `curl` or `wget` are sometimes faster than in-game downloads.
  - Cannot run auto updates on background.
  - Need to wait or entirely cancel the updates while opening the game.
- No options to *cancel*, *pause*, or *resume* downloads in the in-game mod menu when resolving missing dependencies.
- Lack of clarity about `dependencies` when uninstalling mods.

---

## Bug Reports

If you encounter any issues or bugs, please open an issue using the bug report template:

[Submit Bug Report](https://github.com/pinpinroku/hultra/issues/new?assignees=&labels=&template=bug_report.md&title=)

---

## License

This project is licensed under the [GPLv3 license](https://github.com/pinpinroku/hultra/blob/main/LICENSE).

---

## Acknowledgments

This project was made possible thanks to the [EverestUpdateCheckerServer](https://github.com/maddie480/EverestUpdateCheckerServer) hosted by [maddie480](https://github.com/maddie480). We're grateful for their work, which has been instrumental in the development of this project.

---

## Contact

For questions or support, reach out via email or open a discussion in the [Discussions tab](https://github.com/pinpinroku/hultra/discussions/new?category=q-a).
