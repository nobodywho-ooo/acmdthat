{
  description = "acmdthat — a local-LLM shell command generator behind an approval gate";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          onnxruntime = pkgs.callPackage ./nix/onnxruntime.nix { };
          acmdthat = pkgs.callPackage ./nix/package.nix {
            inherit onnxruntime;
          };
        in
        {
          default = acmdthat;
          inherit acmdthat;
        }
      );

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.system}.default}/bin/acmdthat";
        };
      });

      devShells = forAllSystems (
        pkgs:
        let
          onnxruntime = pkgs.callPackage ./nix/onnxruntime.nix { };
        in
        {
          default = pkgs.callPackage ./nix/shell.nix { inherit onnxruntime; };
        }
      );

      # TODO(M1): checks with a tiny GGUF so `nix flake check` exercises inference
    };
}