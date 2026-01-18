# CHANGELOG

Changes from `v3.x` to `v4`.

## BREAKING CHANGES

* The repository name becomes `hultra` from `everest-mod-cli`. It should indicate more explicit that this tool does not actually related to `Everest`. The new name feels more unique and easy to type!
* The `LICENSE` of this project changed to `GPLv3` from `MIT`
* The `install` command now accepts multiple URLs at once. No need to install mods one by one
* The `update` command now runs without any arguments. It automatically updates all mods
* New option `--use-api-mirror` is introduced. Now we can use mirror for prior database fetching
* File hash caching is implemented. It will saved to `~/.local/state/hultra/checksum.cache` once it hashed

## Changed

* (internal): Upgrade `reqwest` to v0.13.1 which introduces breaking changes to TLS backend. Now we can use rustls by default without modifying it manually
* Remove `anyhow` from every modules, restrict its usage in main and test cases
* Add custom error types for every modules using `thiserror` instead of simple errors using `anyhow` for more robust error handling
* Extern crate `mirror-list` now integrates in main source directly: `./src/mirrorlist.rs` 
* (rename): Extern crate `zip-search` now renamed to `zip-finder` for clarity

## Fixed

* Duplicate downloads for the mod which have `everest.yml` instead of `.yaml`. Now `everest.yml` can be parsed as well as `everest.yaml`

## Added

* Implement a spinning indicator for prior database fetching
* Add more logs and refine them to get information what actually we need
* Implement `fmt::Display` for installed mod instead of manual formatting in main function

## Improved

* (zip): Improve file finding strategy in ZIP archive. Significantly lowers memory usage and improve finding speed
* Hash calculation now only runs when actually it is required instead of using `OnceCell`
* Calculated hash will be cached, so update command completes more faster than before

## Foot Notes

We are going to focus on the features that install and update mods from now. So we do not introduce or implement new features that manages local mods. We may create another project to manage mods like dependency resolving or orphan findings.
