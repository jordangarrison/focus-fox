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
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # notify-send for phase-change notifications
        runtimeDeps = with pkgs; [ libnotify ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain ] ++ runtimeDeps;
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "focus-fox";
          version = "0.1.0";
          src = ./.;
          cargoLock = { lockFile = ./Cargo.lock; };

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postInstall = ''
            for bin in focus-fox fox; do
              wrapProgram $out/bin/$bin \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
            done
          '';

          meta = with pkgs.lib; {
            description = "Terminal-based pomodoro timer";
            license = licenses.mit;
            mainProgram = "focus-fox";
          };
        };
      }
    );
}
