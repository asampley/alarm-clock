{
  description = "UTF-Nate";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    systems = {
      url = ./systems.nix;
      flake = false;
    };
  };
  outputs =
    inputs:
    let
      systems = import inputs.systems;
      lib = inputs.nixpkgs.lib;
      genSystems = lib.genAttrs systems;
      pkgsFor =
        localSystem: crossSystem:
        import inputs.nixpkgs {
          localSystem = {
            system = localSystem;
          };
          crossSystem = {
            system = crossSystem;
          };
        };
      shell =
        { pkgsBuildHost, ... }:
        pkgsBuildHost.mkShell {
          buildInputs = with pkgsBuildHost; [
            espup
            just
            rustup
            probe-rs-tools
          ];


          shellHook = ''
            espup install
            . ~/export-esp.sh
          '';
        };
    in
    {
      formatter = genSystems (system: (pkgsFor system system).nixfmt-rfc-style);

      devShells = genSystems (system:
        lib.mergeAttrsList (
          map (cross: {
            default = (pkgsFor system cross).callPackage shell { };
          }) systems
        )
      );
    };
}
