# acmdthat

> **a** **cmd** **that**… — describe what you want; a local LLM writes the
> shell command; you approve; it runs.

```text
$ acmdthat search for the string 'Asbjørn' in this directory
⠋ thinking · Qwen3.5-4B · GPU…
┌────────────────────────────────────────────────────────────┐
│ $ rg 'Asbjørn'                                               │
├────────────────────────────────────────────────────────────┤
│ Search for the literal string Asbjørn in the current        │
│ directory with ripgrep.       risk: LOW                     │
├────────────────────────────────────────────────────────────┤
│  [r] run   [e] edit   [c] copy   [g] regenerate   [q] quit │
└────────────────────────────────────────────────────────────┘
```

- **Local-first**: inference runs in-process via
  [NobodyWho](https://github.com/nobodywho-ooo/nobodywho) (llama.cpp,
  Vulkan/Metal offload). No API keys, no server, no telemetry. Works offline
  after the first model download.
- **Guaranteed-parseable suggestions**: generation is constrained by a JSON
  schema (llguidance), so even small local models produce structured,
  extractable commands — never prose fences to regex.
- **Approval gate**: nothing executes without an explicit keystroke.
  Deterministic risk heuristics (independent of the LLM) flag dangerous
  patterns before you see the card.
- **Your shell, not POSIX**: commands are generated for `$SHELL` (fish, zsh,
  bash, …) and executed through it.

## Status

- **M0** — scaffold, `cargo build` + `nix build` green with the full
  NobodyWho/llama.cpp dependency tree. ✅ *done 2026-11-16* — see
  `2026-11-16-workflow-log.md` session 2 for the four nix-sandbox fixes
  (oniguruma vendoring, CMAKE_PREFIX_PATH, SPIRV-Headers, NIX_LDFLAGS) and the
  crate2nix-vs-buildRustPackage decision.
- **M1** — minimal end-to-end: model load (progress UI) → constrained
  suggestion → confirm → `$SHELL -c` execution with live output. 🚧
- **M2** — pretty ratatui inline-viewport approval gate (streamed tokens,
  syntax-highlighted card, risk badge, run/edit/copy/regenerate keys). 🔜
- **M3** — expansion: explain mode, refine loop, history context, correction
  mode, bashkit dry-run preview, voice, command-tuned GGUF. 🔜

Design notes live in [`./notes`](./notes). Prior-art catalogue:
[`notes/2026-11-16-prior-art-research.md`](notes/2026-11-16-prior-art-research.md).
Plan: [`notes/2026-11-16-implementation-plan.md`](notes/2026-11-16-implementation-plan.md).

## Build

```sh
# nix (provides everything, incl. cmake/vulkan/bindgen/onnxruntime for the llama.cpp build)
nix build
nix run . -- search for the string 'Asbjørn' in this directory

# or plain cargo (needs cmake, libclang, vulkan headers, shaderc on the host)
nix develop -c cargo build --release
```

Models: the default is **Gemma4-E4B Q4_K_M** (~5.0 GB — the eval-sweep winner on
an 8 GB GPU; see [`evals/RESULTS.md`](evals/RESULTS.md): best exact score,
tied-best program score, zero rambling, 1.4 s median generation on the RX 7700S).
Override with `--model`: `auto` (NobodyWho's memory-based pick), a local `.gguf`
path, or any `hf://user/repo/file.gguf` reference. First use downloads from
Hugging Face into the NobodyWho cache.

## License

EUPL-1.2 (matching NobodyWho). `nobodywho` core is EUPL-1.2; llama.cpp /
llama-cpp-rs are MIT. The `kokoro` TTS feature of nobodywho (which would pull
GPL espeak-ng) is disabled (`default-features = false`).