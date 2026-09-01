{
  description = "alarm-clock";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-pkgset = {
      url = "github:szlend/nix-pkgset";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    xtensa-gcc-x86_64-linux-bin = {
      url = "https://github.com/espressif/crosstool-NG/releases/download/esp-16.1.0_20260609/xtensa-esp-elf-16.1.0_20260609-x86_64-linux-gnu.tar.xz";
      flake = false;
    };
    xtensa-rust-src = {
      url = "https://github.com/esp-rs/rust-build/releases/download/v1.97.0.0/rust-src-1.97.0.0.tar.xz";
      flake = false;
    };
    xtensa-rust-x86_64-linux-bin = {
      url = "https://github.com/esp-rs/rust-build/releases/download/v1.97.0.0/rust-1.97.0.0-x86_64-unknown-linux-gnu.tar.xz";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nix-pkgset,
      nixpkgs,
      fenix,
      xtensa-gcc-x86_64-linux-bin,
      xtensa-rust-src,
      xtensa-rust-x86_64-linux-bin,
      ...
    }:
    let
      # Systems to produce flake outputs for
      forBuildSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
      forHostSystems = nixpkgs.lib.genAttrs (nixpkgs.lib.systems.doubles.all);
    in
    {
      # Nix formatter, called by nix fmt, change to whatever you'd like.
      formatter = forBuildSystems (system: nixpkgs.legacyPackages.${system}.nixfmt-tree);

      # Exposes packages that are defined in makePkgs. Leave as is.
      legacyPackages = forBuildSystems (system: self.lib.makePkgs nixpkgs.legacyPackages.${system});

      # Expose packages where build == host. Leave as is.
      packages = forBuildSystems (
        system: nixpkgs.lib.filterAttrs (_: nixpkgs.lib.isDerivation) self.legacyPackages.${system}
      );

      platforms = {
        xtensa-esp32s3-none-elf = {
          config = "xtensa-esp32s3-none-elf";
          parsed = {
            cpu = {
              name = "xtensa";
            };
          };
          isx86_32 = false;
          isNone = true;
          isElf = true;
          isRiscV = false;
          isLoongArch64 = false;
          rust = {
            cargoEnvVarTarget = "XTENSA_ESP32S3_NONE_ELF";
            cargoShortTarget = "xtensa-esp32s3-none-elf";
            isNoStdTarget = true;
            platform = {
              arch = "xtensa";
              env = "elf";
              os = "none";
              vendor = "esp32s3";
            };
            rustcTarget = "xtensa-esp32s3-none-elf";
            rustcTargetSpec = "xtensa-esp32s3-none-elf";
          };
          rustc.targetPlatforms = "xtensa-esp32s3-none-elf";
        };
      };

      # Expose dev shells where build == host. Leave as is
      devShells = forBuildSystems (
        system:
        let
          myPkgs = (self.lib.makePkgs nixpkgs.legacyPackages.${system});
        in
        {
          default = myPkgs.callPackage nix/shell.nix { };
          xtensa = myPkgs.callPackage nix/shell.nix {
            alarm-clock = myPkgs.cross.xtensa-esp32s3-none-elf.alarm-clock;
          };
        }
      );

      # Function to make cross aware package set like nixpkgs
      # myPkgs contains a callPackage function like nixpkgs to support
      # all the cross compilation facilities built into nixpkgs.
      # in theory these can then be chained with multiple wrapping newScope
      # calls.
      #
      # Is a function to allow consumers to use a different nixpkgs for cross compiling.
      lib.makePkgs =
        pkgs:
        nix-pkgset.lib.makePackageSet "pkgs" pkgs.newScope (
          myPkgs:
          {
            # Specify your packages here. They will have all cross compilation that nixpkgs has.
            # These can be build using flakes like `nix build .#cross.<crossSystem>.myPackage

            default = myPkgs.callPackage nix/alarm-clock.nix { };
            alarm-clock = myPkgs.callPackage nix/alarm-clock.nix { };

            xtensa-rust-src = myPkgs.callPackage nix/xtensa-rust-src.nix { src = xtensa-rust-src; };

            cargo = myPkgs.rustToolchain;
            rustc = myPkgs.rustToolchain;

            rustToolchain = myPkgs.callPackage (
              { stdenv }:
              let
                fenix' = fenix.packages.${stdenv.buildPlatform.system};
              in
              with fenix';
              combine [
                minimal.cargo
                minimal.rustc
                targets.${stdenv.hostPlatform.rust.rustcTarget}.latest.rust-std
              ]
            ) { };

            rustPlatform = myPkgs.callPackage (
              { stdenv }:
              pkgs.makeRustPlatform {
                inherit stdenv;
                cargo = myPkgs.rustToolchain;
                rustc = myPkgs.rustToolchain;
              }
            ) { };

            cross =
              (forHostSystems (
                hostSystem:
                self.lib.makePkgs (
                  import nixpkgs {
                    localSystem = pkgs.stdenv.buildPlatform;
                    crossSystem = hostSystem;
                  }
                )
              ))
              // {
                xtensa-esp32s3-none-elf =
                  (self.lib.makePkgs (
                    import nixpkgs {
                      localSystem = pkgs.stdenvNoCC.buildPlatform;
                      #config.allowUnsupportedSystem = true;
                      config.replaceStdenvNoCC =
                        { pkgs, ... }:
                        pkgs.stdenvNoCC.override {
                          hostPlatform = self.platforms.xtensa-esp32s3-none-elf;
                          targetPlatform = self.platforms.xtensa-esp32s3-none-elf;
                        };
                    }
                  )).overrideScope
                    (
                      prev: final: {
                        alarm-clock = prev.callPackage nix/xtensa-alarm-clock.nix { };
                        rustToolchain = prev.callPackage (
                          { symlinkJoin }:
                          symlinkJoin {
                            name = "xtensa-rust-with-src";
                            paths = [
                              final.xtensa-rust
                              final.xtensa-rust-src
                            ];
                          }
                        ) { };
                        stdenvNoCC = pkgs.stdenvNoCC // {
                          hostPlatform = self.platforms.xtensa-esp32s3-none-elf;
                          targetPlatform = self.platforms.xtensa-esp32s3-none-elf;
                        };
                        stdenv = pkgs.stdenv // {
                          cc = myPkgs.xtensa-gcc;
                          hostPlatform = self.platforms.xtensa-esp32s3-none-elf;
                          targetPlatform = self.platforms.xtensa-esp32s3-none-elf;
                        };
                      }
                    );
              };
          }
          // (
            if pkgs.stdenv.buildPlatform.config == "x86_64-unknown-linux-gnu" then
              {
                xtensa-gcc = myPkgs.callPackage nix/xtensa-gcc.nix { inherit xtensa-gcc-x86_64-linux-bin; };
                xtensa-rust = myPkgs.callPackage nix/xtensa-rust.nix {
                  xtensa-rust-archive-bin = xtensa-rust-x86_64-linux-bin;
                };
              }
            else
              { }
          )
        );
    };
}
