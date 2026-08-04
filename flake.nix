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
        audioDevDeps = lib.optionals isLinux [ pkgs.pkg-config pkgs.alsa-lib ];

        mkFocusFox = rustPlatform: alsaLib: rustPlatform.buildRustPackage {
          pname = "focus-fox";
          inherit version;
          src = ./.;
          cargoLock = { lockFile = ./Cargo.lock; };
          nativeBuildInputs = lib.optionals isLinux [ pkgs.pkg-config ];
          buildInputs = lib.optionals isLinux [ alsaLib ];

          meta = with lib; {
            description = "Terminal-based pomodoro timer";
            homepage = "https://github.com/jordangarrison/focus-fox";
            license = licenses.mit;
            mainProgram = "focus-fox";
          };
        };

        # dynamically linked build for nix users
        unwrapped = mkFocusFox pkgs.rustPlatform pkgs.alsa-lib;

        # Downloadable Linux binaries run outside the Nix store. Build the
        # statically linked ALSA client against standard FHS data/plugin paths
        # so it uses the target distribution's ALSA configuration instead of
        # embedding references to Nix's build-time alsa-lib output.
        portableAlsaLib = pkgs.pkgsStatic.alsa-lib.overrideAttrs (old: {
          # Configure must retain Nix output paths for installation. Change
          # only constants compiled into libasound after configure completes.
          postConfigure = (old.postConfigure or "") + ''
            sed -i \
              -e 's|^#define ALSA_CONFIG_DIR .*|#define ALSA_CONFIG_DIR "/usr/share/alsa"|' \
              -e 's|^#define ALSA_PLUGIN_DIR .*|#define ALSA_PLUGIN_DIR "/usr/lib/alsa-lib"|' \
              include/config.h
          '';
        });

        # fully static musl build — the portable binary that goes into the
        # deb/rpm/arch packages and the tarball (linux only)
        static = (mkFocusFox pkgs.pkgsStatic.rustPlatform portableAlsaLib).overrideAttrs (old: {
          # Fail the build if ALSA's Nix-store locations leak back into the
          # supposedly relocatable release binary.
          postFixup = (old.postFixup or "") + ''
            rm -f $out/nix-support/propagated-build-inputs
          '';
          disallowedReferences = (old.disallowedReferences or [ ]) ++ [
            portableAlsaLib
            (lib.getDev portableAlsaLib)
          ];
        });

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

        mkNfpmConfig = dependencies: pkgs.writeText "nfpm.yaml" ''
          name: focus-fox
          arch: ${goArch}
          platform: linux
          version: "${version}"
          section: utils
          maintainer: Jordan Garrison <jordangarrison@users.noreply.github.com>
          description: Terminal-based pomodoro timer
          homepage: https://github.com/jordangarrison/focus-fox
          license: MIT
          depends: [${lib.concatStringsSep ", " dependencies}]
          contents:
            - src: ${static}/bin/focus-fox
              dst: /usr/bin/focus-fox
            - src: ${static}/bin/fox
              dst: /usr/bin/fox
        '';

        mkNfpmPackage = format: dependencies:
          let nfpmConfig = mkNfpmConfig dependencies;
          in pkgs.runCommand "focus-fox-${version}-${format}"
          { nativeBuildInputs = [ pkgs.nfpm ]; } ''
          mkdir -p $out
          nfpm package -f ${nfpmConfig} -p ${format} -t $out
        '';

        linuxPackages = lib.optionalAttrs (isLinux && goArch != null) {
          inherit static;
          deb = mkNfpmPackage "deb" [ "libasound2-data" ];
          rpm = mkNfpmPackage "rpm" [ "alsa-lib" ];
          arch = mkNfpmPackage "archlinux" [ "alsa-lib" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain ] ++ runtimeDeps ++ audioDevDeps;
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
