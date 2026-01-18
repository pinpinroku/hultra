# Zip Finder

A utility to find and decode target file in ZIP archive without extracting the entire archive.

This is achieved by searching for the EOCD (End of Central Directory) and obtaining the offset to the target file. Then returns decompressed file content as bytes.

## Features

- Does not extract the whole archive to find a specific file
- Only extracts the necessary parts of the binary to get the file contents
- File existence checks are more lightweight as they only examine the CDFH records

## Motivation

Why was this tool implemented?

There is a well-known Rust library for managing ZIP files called `zip` (zip-rs). This library creates an index of the entire archive by scanning the complete binary. This takes too much time when the user only needs a single specific file. The initialization cost is very high.

## Limitations

- Only supports single-disk archives (multi-disk/split archives are not supported)
- No ZIP64 support
- No encryption support
