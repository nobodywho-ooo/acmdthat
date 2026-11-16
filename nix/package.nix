# acmdthat — the package.
#
# Rust + the full NobodyWho dependency tree: llama-cpp-sys-2 builds llama.cpp
# with cmake in-sandbox (Vulkan backend → vulkan-headers/shaderc/mesa), and
# ort-sys links pyke's prebuilt ONNX Runtime via ORT_LIB_PATH. The git
# dependencies (nobodywho core + llama-cpp-rs) are prefetched with submodules
# through cargoLock.outputHashes — nixpkgs fetchgit defaults
# fetchSubmodules=true, which llama-cpp-sys-2 needs (llama.cpp is a submodule
# of llama-cpp-rs).
{
  lib,
  stdenv,
  rustPlatform,
  cmake,
  pkg-config,
  makeWrapper,
  llvmPackages,
  vulkan-headers,
  vulkan-loader,
  shaderc,
  mesa,
  addDriverRunpath, # patches RUNPATH so ggml-vulkan's dlopen("libvulkan.so.1") finds the system loader
  spirv-headers, # llama.cpp ggml-vulkan: find_package(SPIRV-Headers CONFIG REQUIRED)
  oniguruma, # onig_sys (via nobodywho's tokenizers) links this
  onnxruntime, # from ./onnxruntime.nix
}:
let
  cargoToml = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "acmdthat";
  inherit (cargoToml.package) version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
    ];
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
    # Git deps: keyed "<crate>-<version>", hashed by the rev they resolve to.
    # llama-cpp-2 and llama-cpp-sys-2 share a rev; nobodywho and gbnf share one.
    # Update these with `nix-prefetch-git --fetch-submodules <url> <rev>`
    # (+ `nix hash convert --hash-algo sha256 --from nix32 --to sri`) when
    # bumping the pinned revs in Cargo.toml.
    outputHashes = {
      "nobodywho-2.4.0" = "sha256-7+0OZ/PzW3Zt2A8XNzyylRkzRR3AvPHjt8hc9PDmEKM=";
      "gbnf-0.1.0" = "sha256-7+0OZ/PzW3Zt2A8XNzyylRkzRR3AvPHjt8hc9PDmEKM=";
      "llama-cpp-2-0.1.161" = "sha256-7HLuhEYdg2kUzgyrYYFDgiA8mTeYZ9HCjypoRB1a70k=";
      "llama-cpp-sys-2-0.1.161" = "sha256-7HLuhEYdg2kUzgyrYYFDgiA8mTeYZ9HCjypoRB1a70k=";
    };
  };

  # llama-cpp-sys-2 (llama.cpp via cmake) + ort-sys (bindgen) build needs.
  nativeBuildInputs = [
    cmake
    pkg-config
    rustPlatform.bindgenHook
    llvmPackages.libclang
    vulkan-headers
    vulkan-loader
    shaderc
    mesa
    # patch the binary's RUNPATH so libvulkan.so.1 (dlopen'd by ggml-vulkan)
    # resolves against the system driver dir
    addDriverRunpath
    oniguruma
  ];

  env = {
    # bindgen
    LIBCLANG_PATH = "${lib.getLib llvmPackages.libclang}/lib";
    # ort-sys: prebuilt ONNX Runtime instead of downloading in-sandbox
    ORT_LIB_PATH = "${onnxruntime}/lib";
    # llama.cpp's FindVulkan probe: buildRustPackage runs build scripts
    # in-tree without the cmake setup hook's CMAKE_PREFIX_PATH collection,
    # and the sandbox has no system include dirs. llama-cpp-sys-2's build.rs
    # forwards CMAKE_* env vars as -D defines, so hand it the store paths.
    CMAKE_PREFIX_PATH = lib.concatStringsSep ":" (
      map lib.getDev [
        vulkan-headers
        shaderc
        spirv-headers
        mesa
      ]
      # the loader's dev output has only cmake/pkgconfig files; libvulkan.so
      # is in the out output
      ++ [ (lib.getLib vulkan-loader) ]
    );
    # llama.cpp's ggml-vulkan C++ does `__has_include(<spirv/unified1/spirv.hpp>)`
    # expecting SPIRV-Headers in default include paths (Vulkan SDK layout) and
    # never links the cmake target; no default include paths exist in the
    # sandbox, so put them on every C++ unit instead (forwarded as -D by
    # llama-cpp-sys-2's build.rs; the cmake crate doesn't define this one).
    CMAKE_CXX_STANDARD_INCLUDE_DIRECTORIES = "${lib.getDev spirv-headers}/include";
    # Final link: the binary links the Vulkan loader (-lvulkan) because
    # llama.cpp was configured with GGML_VULKAN=ON; llama-cpp-sys-2 emits
    # rustc-link-lib=vulkan without a search path for it. cc-wrapper appends
    # NIX_LDFLAGS to every link line. (RUNPATH for runtime dlopen of
    # libvulkan.so.1 is handled by addDriverRunpath in the fixup phase.)
    NIX_LDFLAGS = "-L${lib.getLib vulkan-loader}/lib";
    # llama.cpp CPU features (as in nobodywho's own nix build)
    CARGO_CFG_TARGET_FEATURE =
      if stdenv.hostPlatform.isx86_64 then "sse4.2,fma,avx,avx512" else "";
  };

  # The cargo setup hook forces system libs (RUSTONIG_SYSTEM_LIBONIG=1, ...)
  # on every build script; that forces onig_sys (via nobodywho → tokenizers' `onig`
  # backend) to probe for system oniguruma through pkg-config. Let onig_sys compile
  # its vendored C source instead (sandbox-pure, cc-only). Set back to false and
  # keep oniguruma in nativeBuildInputs if we ever prefer system linking.
  dontCargoUseSystemLibs = true;

  doCheck = true;
  cargoTestFlags = [ "--quiet" ];

  meta = {
    description = "acmdthat <what you want> — local-LLM shell command generator behind an approval gate";
    homepage = "https://github.com/asbjorn/acmdthat"; # TODO: set real repo URL
    license = lib.licenses.eupl12;
    mainProgram = "acmdthat";
  };
}