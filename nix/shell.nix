# acmdthat — devShell.
#
# Mirrors the native dependencies of nix/package.nix so that plain
# `cargo build` / `cargo run` work inside `nix develop`, including
# llama.cpp's cmake build (Vulkan) and ort-sys's ONNX Runtime link.
{
  lib,
  stdenv,
  mkShell,
  rustc,
  cargo,
  rustfmt,
  clippy,
  rust-analyzer,
  rustPlatform,
  cmake,
  pkg-config,
  llvmPackages,
  vulkan-headers,
  vulkan-loader,
  vulkan-tools,
  shaderc,
  mesa,
  spirv-headers,
  oniguruma,
  onnxruntime, # from ./onnxruntime.nix
  ripgrep,
  just,
}:
mkShell {
  packages = [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
    rustPlatform.bindgenHook # exports BINDGEN_EXTRA_CLANG_ARGS (+ LIBCLANG_PATH) for llama-cpp-sys-2's bindgen
    llvmPackages.libclang
    cmake
    pkg-config
    vulkan-headers
    vulkan-loader
    vulkan-tools # vulkaninfo for debugging GPU offload
    shaderc
    mesa
    oniguruma
    ripgrep
    just
  ];

  env = {
    LIBCLANG_PATH = "${lib.getLib llvmPackages.libclang}/lib";
    ORT_LIB_PATH = "${onnxruntime}/lib";
    # rust-analyzer
    RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
    # same FindVulkan/SPIRV fixes as nix/package.nix, for in-tree `cargo build`
    CMAKE_CXX_STANDARD_INCLUDE_DIRECTORIES = "${lib.getDev spirv-headers}/include";
    # same final-link fix as nix/package.nix (-lvulkan at link time)
    NIX_LDFLAGS = "-L${lib.getLib vulkan-loader}/lib";
    CMAKE_PREFIX_PATH = lib.concatStringsSep ":" (
      map lib.getDev [
        vulkan-headers
        shaderc
        spirv-headers
        mesa
      ]
      ++ [ (lib.getLib vulkan-loader) ]
    );
    CARGO_CFG_TARGET_FEATURE =
      if stdenv.hostPlatform.isx86_64 then "sse4.2,fma,avx,avx512" else "";
  };
}