{ lib, stdenv, rustPlatform, installShellFiles, pkg-config, zstd }:

let
  cargoToml = lib.importTOML ../Cargo.toml;
  msrv = cargoToml.package.rust-version;
  rustcVersion = rustPlatform.rust.rustc.version;
in

assert lib.assertMsg (lib.versionAtLeast rustcVersion msrv) ''
  hultra needs Rust >= ${msrv}, but this nixpkgs provides rustc ${rustcVersion}.
  Update nixpkgs, or build with rust-overlay/fenix via pkgs.makeRustPlatform.
'';

rustPlatform.buildRustPackage {
  pname = "hultra";
  inherit (cargoToml.package) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.difference ../. (lib.fileset.unions [
      ../.github
      ../nix
      ../flake.nix
      ../flake.lock
      ../.gitignore
      ../CHANGELOG.md
      ../CONTRIBUTING.md
    ]);
  };

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ installShellFiles pkg-config ];
  buildInputs = [ zstd ];

  # nixpkgs' cargoSetupHook sets AWS_LC_SYS_USE_SYSTEM=1,
  # but if no system AWS-LC is found the build script gives up.
  # So override it in the preBuild phase
  preBuild = ''
    export AWS_LC_SYS_USE_SYSTEM=0
  '';

  postInstall = lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
    installShellCompletion --cmd hultra \
      --bash <($out/bin/hultra generate-completion bash) \
      --zsh  <($out/bin/hultra generate-completion zsh) \
      --fish <($out/bin/hultra generate-completion fish)
  '';

  meta = {
    description = "Command-line installer and updater for Celeste mods";
    homepage = "https://github.com/pinpinroku/hultra";
    license = lib.licenses.gpl3Plus; # gpl-3.0-or-later, matches Cargo.toml
    mainProgram = "hultra";
    platforms = lib.platforms.linux;
  };
}
