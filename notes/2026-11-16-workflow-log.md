---
title: "acmdthat — workflow log"
date: 2026-11-16
tags: [log, workflow]
related:
  - 2026-11-16-prior-art-research.md
  - 2026-11-16-nobodywho-integration-notes.md
  - 2026-11-16-implementation-plan.md
author:
  created-by: pi-coding-agent
  model: z-ai/glm-5.3 (via openrouter)
  harness: pi (@earendil-works/pi-coding-agent v1.5.2)
  host: furnace (NixOS Linux x86_64)
---

# Workflow log

Running log of work sessions on `acmdthat`. Newest entries at the bottom.

## 2026-11-16 — Session 1 (research & planning)

Environment: pi coding agent 1.5.2 on `furnace`, model z-ai/glm-5.3 via
openrouter, reason level high. CWD: `~/Development/suggest` (empty except
`.env` device file — sandbox artifact, and empty `notes/` dir).

Done:

1. **Inspected NobodyWho** (`~/Development/nw/nobodywho`, main @ 2aed9468,
   core 2.4.0): chat API (`ChatBuilder`/`ChatHandle`/`TokenStream`),
   `llm::get_model("auto", …)` memory-based model selection, llguidance
   sampler constraints (`constrain_with_json_schema`), `Tool::bash` bashkit
   sandbox, EUPL-1.2 license, not on crates.io (git dep required,
   subdirectory `nobodywho/core`).
2. **Prior-art research** (3 web-research rounds + repo README fetches):
   catalogue written to `2026-11-16-prior-art-research.md`. Highlights:
   gh-copilot UX is the gold standard; caro is the closest local-first Rust
   sibling (macOS/MLX, AGPL); NL2Shell proves tiny command-tuned models work;
   NVIDIA blog proves grammar-constrained decoding helps small-model bash
   generation; magic-cli is the "don't auto-execute" cautionary tale.
3. **Build research**: confirmed cargo git-dep checkouts include the
   llama.cpp submodule; confirmed nixpkgs `fetchgit` defaults
   `fetchSubmodules = true` (so both crate2nix *and*
   buildRustPackage+cargoLock work for llama-cpp-sys-2). Chose
   buildRustPackage + cargoLock + outputHashes. Noted `ort` is an
   unconditional dep of nobodywho core → expect to mirror nobodywho's
   `ort-sys` nix override (`ORT_LIB_PATH`).
4. **Wrote**: `2026-11-16-prior-art-research.md`,
   `2026-11-16-nobodywho-integration-notes.md`,
   `2026-11-16-implementation-plan.md` (this log).
5. **Next**: design review with Asbjørn via ask_user (UI approach, model
   default, license, approval actions), then M0 scaffold.

Sandbox notes: bash network is fine for crates.io/github/huggingface;
`~/.cargo` is masked from the agent sandbox (workaround: read llama-cpp-rs
sources from GitHub when needed). No blockers so far; will flag if model
download during dev/testing needs an exception.

## 2026-11-16 — Session 2 (M0: build story green)

Environment: same pi harness/host. Decisions from the design review:
**plan as written · M1 simple prompts then M2 ratatui · model = `auto`
(+ evaluate command-tuned GGUFs in M3) · license = EUPL-1.2**.

Done:

1. **Answered "would crate2nix be simpler?"**: no — nobodywho's `Cargo.nix`
   only covers *their* workspace; we'd still generate+commit our own 10k-line
   Cargo.nix via the **third-party fork** `macalinao/fast-crate2nix` (upstream
   has the git-dep workspace-inheritance bug). Their flake also exposes no
   consumable Rust crate output. Stayed on buildRustPackage+cargoLock.
   Both paths share the same `Cargo.lock`, so switching later is cheap.
2. **`nix build .#acmdthat` green** after four sandbox fixes (each explained in
   `nix/package.nix` comments):
   - `dontCargoUseSystemLibs = true` — nixpkgs' `cargo-setup-hook.sh` exports
     `RUSTONIG_SYSTEM_LIBONIG=1`, forcing `onig_sys` (via nobodywho → tokenizers'
     `onig` backend) to find system oniguruma via pkg-config; vendored compile
     instead (crate2nix path never hit this because buildRustCrate doesn't run
     that hook).
   - `env.CMAKE_PREFIX_PATH` = getDev of vulkan-headers/shaderc/spirv-headers/
     mesa **+ getLib vulkan-loader** (libvulkan.so is in `out`, not `dev`!) —
     llama-cpp-sys-2's build.rs forwards `CMAKE_*` env as -D defines; FindVulkan
     found nothing otherwise because buildRustPackage runs no cmake setup-hook
     CMAKE_PREFIX_PATH collection for in-tree build scripts.
   - `spirv-headers` in inputs + `env.CMAKE_CXX_STANDARD_INCLUDE_DIRECTORIES` —
     llama.cpp's ggml-vulkan does `find_package(SPIRV-Headers CONFIG REQUIRED)`
     but **never links the target**; it expects `spirv/unified1/spirv.hpp` via
     `__has_include` in default compiler include paths (Vulkan SDK layout).
     nixpkgs' spirv-headers ships `SPIRV-HeadersConfig.cmake`.
   - `env.NIX_LDFLAGS = "-L…/vulkan-loader…/lib"` — final binary links
     `-lvulkan` (GGML_VULKAN=ON) with no search path; cc-wrapper appends
     NIX_LDFLAGS. Runtime dlopen handled by addDriverRunpath (fixup phase).
3. **`nix develop -c cargo build` green** (5m47s) after mirroring the fixes into
   `nix/shell.nix`: same env vars **+ `rustPlatform.bindgenHook`** (exports
   BINDGEN_EXTRA_CLANG_ARGS incl. libc headers; without it bindgen in
   llama-cpp-sys-2 fails on `stdio.h not found`) + fixed the flake's devShells
   `onnxruntime` plumbing (was a bogus `inherit (callPackage …) onnxruntime`).
4. Verified binaries run: `./result/bin/acmdthat --help/--version/<what>` and
   `./target/debug/acmdthat` both work; `has_gpu_backend()` returns false in
   the agent sandbox (expected — bwrap has no GPU; will be true on the host).

Notes on llama.cpp cmake inside the build: non-fatal `Could NOT find Git` and
`Could NOT find OpenSSL` warnings are expected (build-info string; LLAMA_CURL=OFF).

**M0 status: complete.** Next: M1 minimal end-to-end (model load w/ progress
→ llguidance-JSON-constrained suggestion → simple confirm → `$SHELL -c`
execution w/ live output), then M2 ratatui inline gate.

Pending: user reviews and commits the scaffold (I don't commit).

## 2026-11-16 — Session 3 (M1: end-to-end pipeline implemented)

Implemented M1 end-to-end: `src/context.rs` (OS/shell/cwd listing for the
prompt), `src/suggest.rs` (JSON-schema grammar + system prompt + streamed
generation), `src/risk.rs` (deterministic danger patterns, LLM-independent),
`src/exec.rs` (`$SHELL -c`, live output, exit codes incl. signal mapping),
`src/ui.rs` (styled card + approval gate; high-risk requires typing `run`),
`src/main.rs` (clap, tracing init, model load w/ spinner, --print/--json/-v).

API learnings (for the integration notes):
- Sampler presets are `SamplerPresets::…` NOT `SamplerConfig::…`; composable
  chains go through `SamplerBuilder::new().shift(ShiftStep::…).add_grammar(
  GrammarStep::JsonSchema(schema)).sample(SampleStep::Dist)`. Grammar masks
  before shifts; chain = grammar → top_k → top_p → temperature → Dist.
- `with_allow_thinking` is deprecated → `with_template_variable(
  "enable_thinking".to_string(), false)`.
- `with_system_prompt` takes `Option<S>`.
- `ChatHandle::ask` → `TokenStream` where `completed()` returns the error of the
  stream (needs `Ok(stream.completed()?)` into `miette::Result`).
- cliclack 0.3 `spinner()` returns its own `ProgressBar` (wraps indicatif):
  `start(msg)`, `set_message(msg)`, `stop(msg)` — all take messages.
- **cargo 1.99 auto-discovers git-dep crates by NAME anywhere in the repo tree**
  (per current cargo book: "traverses the file tree to find Cargo.toml for the
  requested crate anywhere inside the git repository") — so the `subdirectory`
  key is legacy/unused (spurious "unused manifest key" warning); removed it,
  resolution to `nobodywho/core` verified via `cargo metadata`.
- nobodywho errors `derive(Debug, thiserror::Error, miette::Diagnostic)` → the
  `nobodywho::miette` re-export gives pretty errors with source chains via
  `fn main() -> miette::Result<()>`.
- Downloads: `DownloadProgressCallback = Arc<dyn Fn(u64,u64)>`; `get_model`
  defaults to nobodywho's indicatif bar (auto-disables non-TTY) when `None`.

Build status: `cargo build` green, zero warnings. `nix build` green.

**Sandbox blocker**: inference end-to-end test blocked — model downloads
redirect to HF CDN hosts (`us.aws.cdn.hf.co`, `cdn-lfs.hf.co`,
`cas-bridge.xethub.hf.co`) which the agent-sandbox proxy 403s. huggingface.co
HTML + github.com work; the CDN backend doesn't. Asked Asbjørn to either add
those domains to the sandbox allowlist, or run the e2e test on the host.
(error rendering itself worked beautifully — miette fancy output with the
full ureq → proxy 403 source chain).

## 2026-11-16 — Session 4 (M1 validated end-to-end in sandbox)

Asbjørn added `*.huggingface.co`-adjacent CDN domains to the sandbox allowlist
("C: both" — sandbox for my iteration + host run to confirm GPU path);
`us.aws.cdn.hf.co` reachable → downloads work. Validation matrix, all green:

| test | result |
|---|---|
| `--print` "ten largest files" | `ls -lR` — valid but weak command; 34s incl. download+load (0.6B, sandbox CPU) |
| `--json` | schema-valid JSON, exit 0 (`grep -iR Asbjørn …` — 0.6B inlined my cwd listing into args; quality is a model-size issue, machinery proven) |
| interactive PTY, approve | spinner w/ live token count → card → `y` → `cd …` ran **via fish** (user's fish config PWD-hooks fired), exit 0 |
| interactive PTY, abort | `n` → "aborted — nothing ran" |
| failing command | model suggested nonexistent `list` → fish exit 127 → `✗ exited with code 127` (exit plumbing works) |
| `cargo test` | 5 risk-pattern tests green |
| `cargo fmt` / `cargo clippy` | clean / zero warnings |

Timing: cached load ≈1s, generation 33 tokens ≈9s on sandbox CPU (~3.7 tok/s).
User's GPU + bigger `auto` model will differ; host run will tell.

Fixes & learnings this session:
- Tracing default filter was `warn` → llama.cpp's benign load-time WARN spam
  (`pooling_type`, control-token type) polluted the UI. Default now `error`;
  `-v` → `info`; RUST_LOG overrides. Verified zero WARN lines.
- Risk unit tests pin the pattern behavior, incl. a **documented false-positive
  class**: quoted literals like `rg 'rm -rf' docs/` rate HIGH — deliberate,
  over-matching is the safe direction (gate asks you to type `run`);
  under-matching a real `rm -rf` would be dangerous.
- cliclack prompts render **and read keys on stderr** — my first PTY test
  redirected stderr to a file → console `not connected`. Test harness lesson.
- `script(1)` runs its command under `$SHELL` → **my test wrappers must use
  fish syntax** (`$status`, not `$?`). Bit me twice. Comedy: my own harness
  tripped on the exact fish-vs-POSIX issue acmdthat exists to solve.
- `cargo test` doesn't relink the plain bin → rebuild before retesting the
  actual binary (stale-binary confusion cost one debug cycle).
- acmdthat runs commands through the user's **full fish config** (zoxide-ish
  `z` PWD hooks fired on `cd`) — intended "your shell, your env" behavior;
  the sandbox noise was read-only-filesystem artifacts. Worth documenting in
  README eventually (hooks run on `cd` etc).

## 2026-11-16 — Session 5 (default model: auto → Qwen3.5-9B)

Asbjørn's host run showed `auto` resolving to **Qwen3.6-27B** (his box has
enough system RAM for the 27B tier) — far too big for an 8 GB GPU; it would
spill and crawl. Decision (his): default to a small fast model instead —
**the latest 9B Qwen = Qwen3.5-9B**.

Chosen artifact: `hf://unsloth/Qwen3.5-9B-GGUF/Qwen3.5-9B-Q4_K_M.gguf`
(5.68 GB — fits 8 GB VRAM with KV headroom). No NobodyWho-vendored 9B exists;
their `-vendor-sampling` GGUFs only embed Qwen's *recommended sampler defaults*
as `general.sampling.*` metadata (upstream GGUFs lack it), and we always pass an
explicit `SamplerConfig`, so a plain unsloth quant is fine. Thinking mode
(Qwen3.5 default-on) stays disabled via our `enable_thinking=false` template
variable.

`--model` default changed; `auto` and any path/hf-ref still override. README
+ `--help` updated. Quality probes on sandbox CPU (~100 s/generation, your GPU
will be much faster), both correct:

| request | suggestion |
|---|---|
| "list the ten largest files in this directory" | `find . -type f -exec du -h {} + \| sort -rh \| head -n 10` |
| "search for the string Asbjørn in this directory, ignoring case" | `rg -i 'Asbjørn' .` |

(0.6B for comparison gave `ls -lR` and a `grep` with the cwd listing inlined —
the 9B is the right size class for the task.)

**M1 status: complete + validated, default model settled.** Pending: host GPU
run of the same flows, then M2 (ratatui inline card, picker, edit/regenerate).
