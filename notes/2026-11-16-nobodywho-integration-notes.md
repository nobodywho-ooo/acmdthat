---
title: "NobodyWho integration notes (API, build systems, licensing)"
date: 2026-11-16
tags: [research, nobodywho, rust, nix, llama-cpp, build]
related:
  - 2026-11-16-prior-art-research.md
  - 2026-11-16-implementation-plan.md
author:
  created-by: pi-coding-agent
  model: z-ai/glm-5.3 (via openrouter)
  harness: pi (@earendil-works/pi-coding-agent v1.5.2)
  host: furnace (NixOS Linux x86_64)
---

# NobodyWho integration notes for `acmdthat`

Source inspected: local checkout at `~/Development/nw/nobodywho`
(git remote `nobodywho-ooo/nobodywho`, branch `main`, rev `2aed9468`,
core crate version `2.4.0` — as of 2026-11-16).

## 1. How to depend on it

- **Not on crates.io.** `nobodywho` on crates.io is `0.0.0-reserved`
  ("official implementation coming soon"). So: **git dependency**:

  ```toml
  [dependencies]
  nobodywho = { git = "https://github.com/nobodywho-ooo/nobodywho", rev = "2aed9468", subdirectory = "nobodywho/core", default-features = false }
  ```

  - Pin `rev` for reproducibility (both cargo and nix).
  - `default-features = false` drops the `kokoro` TTS feature → avoids
    espeak-ng (GPL) linkage. The `nobodywho-server` crate already does this.
  - For hacking against a local checkout, a commented-out
    `[patch."https://github.com/nobodywho-ooo/nobodywho"]` section can point at
    `~/Development/nw/nobodywho/nobodywho/core` — keep it out of the committed
    default so CI/other machines don't need the sibling checkout.

- **Transitive git deps**: `llama-cpp-2` + `llama-cpp-sys-2` come from
  `https://github.com/nobodywho-ooo/llama-cpp-rs.git` (branch
  `lfm-support-hack`, rev `22e58741…` in the current lockfile), and `gbnf`
  comes from the nobodywho repo itself (path dep inside it). All get pinned in
  our `Cargo.lock`. Cargo checks out git-dep **submodules** — confirmed by the
  existing build output referencing
  `~/.cargo/git/checkouts/llama-cpp-rs-…/llama-cpp-sys-2/llama.cpp/…`
  (llama.cpp is a submodule of llama-cpp-rs and is built by llama-cpp-sys-2's
  build.rs via cmake).

## 2. Core API we'll use (all in `nobodywho` crate = `nobodywho/core`)

```rust
use nobodywho::llm;
use nobodywho::chat::ChatBuilder;
use nobodywho::sampler::SamplerConfig;

// Model: "auto" (memory-based pick from NobodyWho's recommended GGUFs:
// Qwen3.5 family / Gemma4 by usable RAM), a local .gguf path, or an hf:// ref.
// Downloads from Hugging Face on first use (cache under dirs::cache_dir()).
// `progress` callback gets download progress.
let model = llm::get_model("auto", /*use_gpu=*/ true, None, None, Some(progress))?;

let chat = ChatBuilder::new(Arc::new(model))
    .with_context_size(4096)              // small ctx is fine for command gen
    .with_system_prompt(SYSTEM_PROMPT)
    .with_allow_thinking(false)           // speed: no <think> blocks
    .with_sampler(SamplerConfig::temperature(0.2)?)  // see below
    .build()?;                             // -> ChatHandle

let mut stream = chat.ask(user_prompt)?;  // -> TokenStream
while let Some(tok) = stream.next_token()? { /* live TUI rendering */ }
```

- `ChatHandle` is `Clone`, cheap, thread-safe; `ask` blocks via
  `TokenStream::next_token()` (there's also `build_async()` →
  `ChatHandleAsync` if we want async).
- `llm::get_cached_models()` — list already-downloaded models.
- `llm::has_gpu_backend()` — Vulkan/Metal availability.
- `nobodywho::send_llamacpp_logs_to_tracing()` — route llama.cpp logs into
  `tracing` (we can gate behind a `-v` flag).

### Sampler / constrained generation (the killer feature)

`nobodywho::sampler::SamplerConfig` supports (via llama.cpp's vendored
llguidance):

- `temperature(t)`, `top_k`, `top_p`, `greedy()`, `dry()` (anti-repetition),
- **`constrain_with_json_schema(schema_string)`** ← use this for
  `{"commands":[{"command":"…","explanation":"…"}]}`-shaped suggestions,
- `constrain_with_regex(pattern)`, `constrain_with_grammar(lark)`.

Grammar-constrained decoding for *bash* is measurably effective on small
models (NVIDIA AI Red Team 2026 blog), and nobodywho ships the machinery.
Check how `SamplerConfig` composes (temperature + constraint) when
implementing; the server crate only uses plain samplers so we'll read
`sampler.rs` again at implementation time.

### Tools (expansion phase, but notable)

`nobodywho::tool_calling::Tool` includes:

- `Tool::python(...)` — monty-sandboxed Python snippet runner,
- `Tool::bash(max_commands, …)` — **bashkit** sandbox: `InMemoryFs` +
  `ExecutionLimits`. → enables a future "dry-run preview" feature: let the
  model *try* the command in an in-memory filesystem and show predicted
  output before the user approves the real execution. No other NL→cmd tool
  has this.

## 3. Nix build (`nix build`)

Constraint: llama-cpp-sys-2 builds llama.cpp via **cmake** in-sandbox; git
deps must be prefetched by nix, including the llama.cpp submodule.

- **nixpkgs `fetchgit` defaults `fetchSubmodules = true`** (verified in
  nixpkgs `pkgs/build-support/fetchgit/default.nix`, 2026-11). Both
  `crate2nix`'s generated `pkgs.fetchgit` (this is how nobodywho's own flake
  builds work) and `rustPlatform.buildRustPackage`'s
  `cargoLock { outputHashes = … }` path (via `importCargoLock`, which calls
  plain `fetchgit`) therefore **do** fetch the llama.cpp submodule. 
- **Chosen approach**: plain `rustPlatform.buildRustPackage` +
  `cargoLock.lockFile` + `cargoLock.outputHashes` entries for the git
  packages (`nobodywho-2.4.0`, `llama-cpp-2`, `llama-cpp-sys-2` — keyed by
  name-version, hashed by rev). Simpler and more standard than crate2nix;
  nobodywho's flake proves the dep tree builds in-sandbox with the right
  nativeBuildInputs.
- **nativeBuildInputs needed for llama-cpp-sys-2** (cribbed from
  `nobodywho/workspace.nix`):
  `cmake`, `rustPlatform.bindgenHook`, `libclang` (via `LIBCLANG_PATH`),
  `llvmPackages.bintools`, `vulkan-headers`, `vulkan-loader`, `shaderc`,
  `mesa` (llvmpipe software Vulkan for nix-side `cargo test`), `git`.
  Plus `addOpenGLRunpath` so the built binary finds `libvulkan` at runtime.
- x86_64 CPU features: nobodywho sets
  `CARGO_CFG_TARGET_FEATURE = "sse4.2,fma,avx,avx512"` in nix builds.
- `ort`/ONNX (whisper etc.): only built if `ort` ends up in our dep graph.
  With `default-features = false` and only chat/sampler/llm used, ort is
  still a dependency of the crate (it's a normal dep of core). If needed:
  nobodywho has `onnxruntime.nix` + `ORT_LIB_PATH` overrides; copy those if
  the build asks for them. **Flag: verify whether core compiles without
  `ort` — Cargo.toml makes `ort` an unconditional dependency, so expect to
  mirror the `ort-sys` override (`env.ORT_LIB_PATH` + `mkdir -p examples deps`).**

## 4. Licensing (important for distribution)

- nobodywho core is **EUPL-1.2** (weak-ish copyleft, compatible with GPL-3;
  static Rust linking of EUPL code into a distributed binary requires the
  combined work to be offerable under EUPL).
- llama.cpp / llama-cpp-rs: MIT.
- `kokoro` default feature pulls **espeak-ng (GPL-3)** — avoided by
  `default-features = false`.
- Decision needed (see design review): license `acmdthat` as EUPL-1.2 to
  keep distribution simple, or keep it private for now.

## 5. Runtime UX facts

- First run downloads the selected GGUF from
  `huggingface.co` (`NobodyWho/*` repos, e.g. Qwen3.5-4B-Q4_K_M ≈ 2.5 GB,
  0.6B ≈ 400 MB). Progress callback exists; we must surface it nicely
  (spinner + MB counter) and survive Ctrl-C.
- `auto` model selection thresholds (from `model_selection.rs`): 0→Qwen3-0.6B,
  1 GiB→Qwen3.5-0.8B, 2→2B, 4→4B, 5→Gemma4-E2B, 7→E4B, 10→12B, 24→27B
  (all Q4_K_M, NobodyWho-vendored GGUFs).
- User's shell on this host is **fish** — the model must be told which shell
  it targets, and execution should use `$SHELL -c` (fish syntax!), not
  hard-coded POSIX.
