# CHANGELOG

Changes from `v3.x` to `v4`.

## BREAKING CHANGES

* Tepository name becomes `hultra` from `everest-mod-cli`. It should indicate more explicit that this tool does not actually related to `Everest`. The new name feels more unique and easy to type!
* The `LICENSE` of this project changed to `GPLv3` from `MIT`.
* The `install` command now accepts multiple URLs at once. No need to install mods one by one.`everest-mod-cli install URL` -> `hultra install URL1 URL2 URL3`
* The `update` command now runs without any arguments. It automatically updates all mods. `everest-mod-cli update --install` -> `hultra update`
* New option `--use-api-mirror` is introduced. Now we can use mirror for prior database fetching.
* Cache of file hash implemented. It will saved to `~/.local/state/hultra/checksum.cache` once it hashed.

## Changed

* (internal): Upgrade `reqwest` to v0.13.1 which introduces breaking changes to TLS backend. Now we can use rustls by default without modifying it manually.
* remove `anyhow` from every modules, restrict its usage in main and test cases
* add custom error types for every modules using `thiserror` instead of simple errors using `anyhow` for more robust error handling
* extern crate `mirror-list` now integrates in main source directly: -> `./src/mirrorlist.rs` 
* (rename): extern create `zip-search` now renamed to `zip-finder` for clarity

## Fixed

* duplicate downloads for the mod which have `everest.yml` instead of `.yaml`. now `everest.yml` can be parsed as well as `everest.yaml`

## Added

* implement spinner indicates waiting while fetching
* add more logs and refine them to get information what actually we need
* implement `fmt::Display` for installed mod instead of manual formatting in main function
* (doc): add references for API: `everest.yaml` and `everest_update.yaml`

## Improved

* (zip): improve file finding strategy in ZIP archive. Significantly lowers memory usage and improve finding speed
* hash calculation now only runs when actually it is required instead of using `OnceCell`
* calculated hash will be cached, so update command completes more faster than before

## Foot Notes

We are going to focus on the features that install and update mods from now. So we do not introduce or implement new features that manages local mods. We may create another project to manage mods like dependency resolving or orphan findings.
