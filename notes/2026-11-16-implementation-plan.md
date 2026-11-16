---
title: "acmdthat — implementation plan and design alternatives"
date: 2026-11-16
tags: [planning, design, rust, tui, nix]
related:
  - 2026-11-16-prior-art-research.md
  - 2026-11-16-nobodywho-integration-notes.md
  - 2026-11-16-workflow-log.md
author:
  created-by: pi-coding-agent
  model: z-ai/glm-5.3 (via openrouter)
  harness: pi (@earendil-works/pi-coding-agent v1.5.2)
  host: furnace (NixOS Linux x86_64)
---

# `acmdthat` — implementation plan

**Pitch:** `acmdthat <what you want>` → a local LLM (NobodyWho) generates a
shell command for *your* shell, shown in a pretty TUI-like card behind an
approval gate; run it, edit it, copy it, or ask for another.

```
$ acmdthat search for the string 'Asbjørn' in this directory
⠋ thinking (Qwen3.5-4B · GPU)…
┌──────────────────────────────────────────────────────────┐
│ rg 'Asbjørn'                                              │
├──────────────────────────────────────────────────────────┤
│ ripgrep search, anchored to the current directory.        │
│ Risk: low · runs: rg                                       │
├──────────────────────────────────────────────────────────┤
│ [r] run  [e] edit  [c] copy  [g] regenerate  [x] explain  │
└──────────────────────────────────────────────────────────┘
```

## Principles

1. **Start minimal**: one-shot NL → command → approve → execute, streaming.
2. **Local-first**: NobodyWho in-process; no server, no keys, offline after
   first model download.
3. **Never execute without explicit approval** (magic-cli's "unsafe-execution"
   default is the cautionary tale; caro's confirm-gate is the norm).
4. **Guaranteed-parseable suggestions**: llguidance JSON-schema-constrained
   generation, so even a 0.8B model yields structured, valid output.
5. Both `cargo build` and `nix build` must work from day one.

## Milestones

- **M0 — Scaffold**: cargo project + flake (buildRustPackage) + devShell +
  notes + a `--help` that parses; CI-able `nix build` with the full dep tree.
- **M1 — Minimal end-to-end**: model load (progress UI) → constrained JSON
  suggestion → simple confirm prompt (cliclack-style) → `$SHELL -c` execute,
  stream stdout/stderr, report exit code. Flags: `--model`, `--cpu`, `--print`
  (no TUI, no execute — pipe-friendly), `--json`.
- **M2 — Pretty TUI approval gate** (ratatui inline viewport): streamed token
  display during generation, command card with shell syntax highlighting
  (syntect), risk badge, keyboard actions (run/edit-in-$EDITOR/copy/
  regenerate/explain), multi-suggestion list.
- **M3 — Expansion grooming** (see roadmap below), prioritized with Asbjørn.

## Architecture

```
acmdthat/
├── src/
│   ├── main.rs        # clap arg parsing → dispatch
│   ├── config.rs      # TOML config (~/.config/acmdthat), model default, flags
│   ├── context.rs     # gather OS, $SHELL, cwd listing, (later: history/git)
│   ├── suggest.rs     # nobodywho: model load, system prompt, JSON-constrained
│   │                  #   sampling, incremental JSON parse of the stream
│   ├── risk.rs        # deterministic danger heuristics (rm -rf, sudo, dd, …)
│   ├── approve.rs     # approval gate UI (M1: simple prompt; M2: ratatui)
│   ├── exec.rs        # $SHELL -c, live output streaming, exit code
│   └── tui/           # M2: ratatui widgets, card view, keybindings
├── flake.nix          # package + devShell
└── notes/             # these documents
```

**Suggestion schema** (llguidance-constrained):

```json
{ "commands": [ { "command": "rg 'Asbjørn'", "explanation": "…" } ],
  "notes": "optional caveats" }
```

Small `max_commands` (1–3) keeps generation fast on tiny models; the primary
suggestion renders big, alternates go in a picker.

**System prompt sketch** (v1): "You convert natural-language requests into
commands for {shell} on {os}. Working directory {cwd} contains: {top-level
ls}. Reply with JSON only… Prefer common tools (rg, fd, jq…). One pipeline,
not a script, unless asked." + 3–5 few-shot pairs. Iterate against a small
NL2Bash-derived eval set later.

**Execution**: `$SHELL -c '<command>'` with inherited stdio, streaming
output; exit code surfaced. Fish users get fish syntax; prompt declares the
shell. (Fallback chain: `$SHELL` → `sh`.)

## Alternatives considered

### A. UI layer

1. **ratatui (`Viewport::Inline`) + crossterm** — renders in the normal
   scrollback, no alt-screen hijack; supports live-updating card, key
   handling, later: suggestion picker + refine chat. Cost: event-loop code;
   blocking `TokenStream` needs a thread + channel (poll with timeout).
2. **cliclack / dialoguer + indicatif** — fastest to pretty; confirm/spinner
   primitives; limited layout control; no streaming-token rendering of our
   own, no card UI.
3. **Full-screen ratatui REPL** (aichat-style) — overkill for one-shot; saves
   for a future `acmdthat chat`.

**Recommendation:** M1 ships (2)-style simple flow to get end-to-end truth
fastest; M2 rebuilds the gate on (1). If we're confident, jump straight to (1)
— decision point for Asbjørn.

### B. Structured output

1. **JSON-schema constraint via llguidance** (chosen): guaranteed parseable;
   evidence (NVIDIA 2026) that constrained decoding helps small models;
   nobodywho ships it; extensible (add fields later without grammar surgery).
2. Plain-text command + separate explanation pass — fragile with small
   models, double latency.
3. GBNF grammar for raw shell only — no explanation channel, brittle quoting
   rules, harder to evolve.
4. nobodywho Tool-calling (`suggest_command` tool) — depends on per-family
   tool-call formats being well-supported by the small model; JSON constraint
   is more universal across GGUFs.

### C. Model selection

1. **`auto`** (nobodywho memory-based selection → Qwen3.5-family Q4_K_M
   GGUFs) as the default, `--model`/config for explicit GGUF/HF ref.
   Chosen: zero-config first run is the whole point of local-first.
2. Fixed default (e.g. Qwen3.5-4B): predictable quality, but wrong for
   machines with 2 GiB free.
3. NL2Shell-style command-tuned fine-tune (GGUF): potentially the best
   latency/accuracy; not yet vendored by NobodyWho → roadmap item: evaluate
   `AryaYT/nl2shell-0.8b` (or NobodyWho-hosted equivalent) as an optional
   "command mode" model.

### D. Build / packaging

1. **buildRustPackage + cargoLock + outputHashes for git deps** (chosen —
   standard nix, submodule-capable via fetchgit, mirrors nobodywho's
   nativeBuildInputs for llama.cpp: cmake, bindgen/libclang, vulkan-headers,
   shaderc, mesa).
2. crate2nix (nobodywho's own approach) — proven for this exact dep tree but
   heavyweight (generated Cargo.nix in-tree).
3. crane — nice, but the cargoLock mechanics above already solve git+submodule;
   no extra value here.
- DevShell mirrors the native deps + rust-analyzer + rustfmt/clippy.
- `cargo build` path: plain cargo with git dep (works offline afterwards).

### E. Safety layers (defense in depth)

1. **Hard gate**: never run without explicit approval (v1 has no `--yes`
   bypass at all; add later if scripting demands it).
2. **Deterministic risk heuristics** (LLM-independent): pattern list
   (sudo, rm -rf / -x, dd of=, mkfs, > /dev/sd*, chmod -R 777 /, fork bombs,
   `curl … | sh`, chown -R /…), colored risk badge + require extra "type the
   word run" confirmation for high-risk commands.
3. Shell-integrity: run via `$SHELL -c` as the user (no elevation), inherit
   env, no hidden `su`/`sudo` in our code.
4. Roadmap: bashkit in-memory dry-run preview; command linting (shellcheck)
   before showing the card.

## Expansion roadmap (post-M2, to prioritize with Asbjørn)

- `acmdthat explain '<cmd>'` — man-page/`--help`-aware explanation.
- Refine loop: "not quite — exclude node_modules" regenerates with context.
- Multi-suggestion picker (schema already supports it).
- Shell history context (Warp-style) + `$SHELL` widget binding (`acmdthat !`
  zsh/fish widget à la Autocomplete.sh).
- Command-correction mode (thefuck-style: previous command + stderr → fix),
  incl. updating shell history after a successful run.
- bashkit dry-run preview (nobodywho's `Tool::bash` in-memory sandbox).
- Voice mode: nobodywho Whisper STT + Silero VAD (vox-style).
- Command-tuned GGUF model option; NL2Bash-subset eval harness in CI.
- `--print`/`--json` scripting polish; shell completions.

## Open decisions → design review

1. M1 UI: simple prompt first (M1 simple → M2 ratatui), or ratatui from day one? **→ A: M1 simple → M2 ratatui (approved 2026-11-16)**
2. Default model policy: `auto` memory-based vs fixed default GGUF? **→ was auto; changed to fixed Qwen3.5-9B Q4_K_M after 27B/auto proved too big for an 8 GB GPU (2026-11-16, session 5); `--model auto` still available**
3. License: EUPL-1.2 vs private vs dual? **→ EUPL-1.2 (approved 2026-11-16)**
4. Approval actions set for v1: run/edit/copy/regenerate/explain — which subset is must-have day one? **→ M1 ships run/quit only (approved 2026-11-16)**
5. Any objection to `~/.config/acmdthat/config.toml` for defaults? **→ none raised; still TODO (M2)**
