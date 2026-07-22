{
  description = "Focus Fox - Terminal-based pomodoro timer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    # no x86_64-darwin: nixpkgs 26.11 dropped the platform
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        inherit (pkgs) lib;
        isLinux = pkgs.stdenv.isLinux;

        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # notify-send for phase-change notifications (linux only; notifications
        # are best-effort at runtime, so darwin just goes without)
        runtimeDeps = lib.optionals isLinux [ pkgs.libnotify ];

        mkFocusFox = rustPlatform: rustPlatform.buildRustPackage {
          pname = "focus-fox";
          inherit version;
          src = ./.;
          cargoLock = { lockFile = ./Cargo.lock; };

          meta = with lib; {
            description = "Terminal-based pomodoro timer";
            homepage = "https://github.com/jordangarrison/focus-fox";
            license = licenses.mit;
            mainProgram = "focus-fox";
          };
        };

        # dynamically linked build for nix users
        unwrapped = mkFocusFox pkgs.rustPlatform;

        # fully static musl build — the portable binary that goes into the
        # deb/rpm/arch packages and the tarball (linux only)
        static = mkFocusFox pkgs.pkgsStatic.rustPlatform;

        # binary shipped in release assets: static on linux, native on darwin
        releaseBin = if isLinux then static else unwrapped;

        tarball = pkgs.runCommand "focus-fox-${version}-tarball" { } ''
          mkdir -p $out
          tar czf $out/focus-fox-${version}-${system}.tar.gz \
            -C ${releaseBin}/bin focus-fox fox
        '';

        # deb/rpm/arch packages via nfpm, from the static binary
        goArch = {
          x86_64-linux = "amd64";
          aarch64-linux = "arm64";
        }.${system} or null;

        nfpmConfig = pkgs.writeText "nfpm.yaml" ''
          name: focus-fox
          arch: ${goArch}
          platform: linux
          version: "${version}"
          section: utils
          maintainer: Jordan Garrison <jordangarrison@users.noreply.github.com>
          description: Terminal-based pomodoro timer
          homepage: https://github.com/jordangarrison/focus-fox
          license: MIT
          contents:
            - src: ${static}/bin/focus-fox
              dst: /usr/bin/focus-fox
            - src: ${static}/bin/fox
              dst: /usr/bin/fox
        '';

        mkNfpmPackage = format: pkgs.runCommand "focus-fox-${version}-${format}"
          { nativeBuildInputs = [ pkgs.nfpm ]; } ''
          mkdir -p $out
          nfpm package -f ${nfpmConfig} -p ${format} -t $out
        '';

        linuxPackages = lib.optionalAttrs (isLinux && goArch != null) {
          inherit static;
          deb = mkNfpmPackage "deb";
          rpm = mkNfpmPackage "rpm";
          arch = mkNfpmPackage "archlinux";
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain ] ++ runtimeDeps;
        };

        packages = {
          inherit tarball;

          default =
            if isLinux then
              unwrapped.overrideAttrs (old: {
                nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ [ pkgs.makeWrapper ];
                postInstall = (old.postInstall or "") + ''
                  for bin in focus-fox fox; do
                    wrapProgram $out/bin/$bin \
                      --prefix PATH : ${lib.makeBinPath runtimeDeps}
                  done
                '';
              })
            else
              unwrapped;

          # everything downloadable for this system in one directory:
          #   nix build .#release
          release = pkgs.symlinkJoin {
            name = "focus-fox-release-${version}";
            paths = [ tarball ] ++ lib.optionals (isLinux && goArch != null) [
              linuxPackages.deb
              linuxPackages.rpm
              linuxPackages.arch
            ];
          };
        } // linuxPackages;
      }
    );
}
