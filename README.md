<div align="center">

# ✨ acmdthat

### *"a cmd that…"* — say what you want, get the shell command.

A local LLM writes the command. You approve it. It runs.<br>
No API keys. No server. No telemetry. Nothing leaves your machine.

![license](https://img.shields.io/badge/license-EUPL--1.2-blue)
![rust](https://img.shields.io/badge/built_with-Rust-orange)
![local](https://img.shields.io/badge/inference-100%25_local-brightgreen)
![version](https://img.shields.io/badge/version-0.1.0-yellow)

</div>

```text
$ acmdthat show the ten largest files under the current directory

◇  model ready: gemma-4-E4B-it-Q4_K_M
◇  thinking done

  ❯ du -ah . | sort -rh | head -n 10
  Calculates disk usage of everything in ., sorts by size in
  human-readable format descending, and shows the top 10.

  risk: low · shell: fish · model: gemma-4-E4B-it-Q4_K_M

◆  Run this command?
│  ○ Yes / ● No
```

<sub>Illustrative transcript: the command and explanation are a recorded answer
from the default model; the prompt chrome is approximated.</sub>

## 🎬 Demo

<p>
  <img src="assets/demo.svg" alt="Terminal recording: acmdthat turns natural-language requests into shell commands, shows a risk assessment, and waits for approval before running anything">
</p>

<sub>A real one-minute session against the [NobodyWho](https://github.com/nobodywho-ooo/nobodywho)
repo: branches sorted by recency and the latest Rust files — each approved at
the gate and run — and finally an `rm -rf` request that trips the high-risk
check and gets interrupted at the prompt, so nothing runs. Everything is
generated and risk-checked on-device by the local default model
(Gemma4-E4B).</sub>

---

## 💡 Why you'll like it

| | |
|---|---|
| 🏠 **Local-first** | Inference runs in-process through [NobodyWho](https://github.com/nobodywho-ooo/nobodywho) (llama.cpp). Works offline after the first model download. |
| 🛑 **Nothing runs without you** | Every command stops at an approval prompt that defaults to **No**. There is no `--yes` flag. |
| ⚠️ **Risk checks that don't trust the model** | Plain regex rules flag `sudo`, `rm -rf`, `curl … \| sh` and friends. High-risk commands make you type `run`. |
| 🐟 **Your shell, not generic POSIX** | Commands are written for `$SHELL` (fish, zsh, bash, …) and executed through it. |
| 🧩 **Always parseable** | Output is grammar-constrained to a JSON schema, so even small models return a clean command, never a wall of prose. |
| ⚡ **Fast on a GPU** | About 1.4 s median per suggestion with the default model on a laptop RX 7700S (see [evals](evals/RESULTS.md)). |

---

## 🚀 Quick start

You need [Nix](https://nixos.org/download) with flakes enabled. Clone this
repo, then:

```sh
# try it without installing
nix run . -- count the lines in README.md

# or install it into your profile
nix profile install .
acmdthat count the lines in README.md
```

> [!NOTE]
> The first run downloads the default model (**~5 GB**) from Hugging Face into
> NobodyWho's cache. After that, it starts in about a second and works offline.
> Short on disk or VRAM? Pick a [smaller model](#-models).

<details>
<summary><b>Building with plain cargo</b></summary>

The dev shell supplies everything the llama.cpp build needs:

```sh
nix develop -c cargo build --release
./target/release/acmdthat count the lines in README.md
```

Without Nix you need cmake, libclang, Vulkan headers and loader, shaderc and
ONNX Runtime on the host. [`nix/shell.nix`](nix/shell.nix) lists the exact set.

</details>

---

## 🎮 Usage

Just type what you want after `acmdthat`:

```sh
acmdthat list all git branches, including remote ones
acmdthat show the last 50 lines of app.log and keep following it
acmdthat make a gzipped tarball of the notes directory called notes.tar.gz
acmdthat "print the value of the 'version' field from package.json, without quotes"
```

You get the command, a one-line explanation, and a risk rating. Confirm and it
runs in your shell with live output. `acmdthat` exits with the command's exit
code.

> [!TIP]
> Your shell reads the request before `acmdthat` does. If it contains quotes,
> `*`, `?`, `&` or `|`, wrap the whole request in double quotes.

### Options

Options go **before** the request. Anything after the first word is treated as
part of the request.

| Flag | What it does |
|---|---|
| `-m`, `--model <MODEL>` | Use another model: a local `.gguf` path, an `hf://` reference, or `auto` (picks by available memory). |
| `--cpu` | Force CPU inference, no Vulkan/Metal offload. |
| `-p`, `--print` | Print the command and exit. No prompt, nothing runs. |
| `--json` | Print the suggestion as JSON and exit. Nothing runs. |
| `-v`, `--verbose` | Show llama.cpp logs on stderr. |
| `-h`, `--help` | Show help. |
| `-V`, `--version` | Show the version. |

### Scripting

`--print` writes the command plus a `#` comment line with the explanation:

```text
$ acmdthat --print "search this directory for the string 'Asbjørn', ignoring case"
rg -i 'Asbjørn' .
# ripgrep searches recursively for 'Asbjørn' ignoring case (-i) in the current directory (.). — 1 suggestion(s), 1.4s
```

`--json` is the cleanest thing to pipe:

```sh
acmdthat --json list all git branches, including remote ones | jq -r '.commands[0].command'
```

```json
{
  "commands": [
    {
      "command": "git branch -a",
      "explanation": "Lists all local and remote branches."
    }
  ]
}
```

When stdout is not a terminal, `acmdthat` refuses to run without `--print` or
`--json`. It will never show an approval prompt on a pipe.

---

## 🛡️ Safety

Three things stand between a suggestion and your system:

1. **The approval prompt.** It defaults to **No**, and saying no prints
   `aborted — nothing ran`.
2. **Risk rules.** A fixed list of patterns is matched against the finished
   command. The model's own opinion of the risk is never consulted.
3. **No hidden privileges.** The command runs as you, through `$SHELL -c`.

| Risk | Triggers | What you have to do |
|---|---|---|
| 🟢 **low** | Everything else | Confirm |
| 🟡 **medium** | `git push --force`, `git reset --hard`, `git clean -f`, `kill` / `killall` / `pkill`, `truncate`, `chmod -R` | Confirm, with the reason shown |
| 🔴 **high** | `sudo`, `rm -r` / `rm -f`, `dd of=`, `mkfs`, `shred`, writing to a raw disk device, `curl`/`wget` piped into a shell, `reboot` / `shutdown`, fork bombs | Type the word `run` |

```text
  ❯ rm -rf build/
  Recursively deletes the build directory.

  risk: high · shell: fish · model: gemma-4-E4B-it-Q4_K_M
  ⚠ recursive/forced delete (rm -r/-f)

◆  DANGEROUS (recursive/forced delete (rm -r/-f)). Type 'run' to execute anyway
```

<sub>Illustrative transcript.</sub>

> [!WARNING]
> The rules are a pattern list, not a sandbox. They err toward over-warning
> (`rg 'rm -rf' docs/` rates high), and a harmful command can still slip past
> as low risk. Read the command before you approve it.

### What the model sees

To write a command that fits your setup, the prompt includes your OS and kernel
version, your shell's name, the current directory path, and up to 60 file names
from its top level. File contents are never read, and all of it stays in the
local process.

---

## 🧠 Models

The default is **Gemma4-E4B (Q4_K_M)**, the winner of the project's 15-case
eval sweep. Swap it with `--model`:

```sh
acmdthat --model hf://NobodyWho/Qwen_Qwen3.5-4B-GGUF/Qwen_Qwen3.5-4B-Q4_K_M-vendor-sampling.gguf list big files
acmdthat --model ~/models/my-model.gguf list big files
acmdthat --model auto list big files
```

| Model | Download | Right tool | Median time | `--model` |
|---|---|---|---|---|
| **Gemma4-E4B** (default) | 4.98 GB | 12/15 | 1.4 s | `hf://NobodyWho/Google_Gemma4-E4B-GGUF/gemma-4-E4B-it-Q4_K_M.gguf` |
| Qwen3.5-4B | 2.74 GB | 12/15 | 2.1 s | `hf://NobodyWho/Qwen_Qwen3.5-4B-GGUF/Qwen_Qwen3.5-4B-Q4_K_M-vendor-sampling.gguf` |
| Qwen3.5-2B | 1.28 GB | 9/15 | 1.0 s | `hf://NobodyWho/Qwen_Qwen3.5-2B-GGUF/Qwen_Qwen3.5-2B-Q4_K_M-vendor-sampling.gguf` |
| Qwen3.5-0.8B | 0.53 GB | 9/15 | 0.5 s | `hf://NobodyWho/Qwen_Qwen3.5-0.8B-GGUF/Qwen_Qwen3.5-0.8B-Q4_K_M-vendor-sampling.gguf` |

"Right tool" counts the cases where the model reached for the same program as
the reference answer. Times were measured on one RX 7700S GPU over Vulkan, one
sample per case, so treat small differences as noise. Full numbers and caveats
are in [`evals/RESULTS.md`](evals/RESULTS.md).

---

## 🔧 Troubleshooting

| Symptom | Fix |
|---|---|
| First run sits there for a long time | It is downloading ~5 GB. Use a smaller model, or wait once. |
| Slow, or GPU errors | Try `--cpu`, or a smaller model that fits your VRAM. |
| `stdout is not a terminal…` | You are piping the output. Add `--print` or `--json`. |
| `Model not found` | Check the `.gguf` path, or use a full `hf://user/repo/file.gguf` reference. |
| The request got mangled | Your shell expanded something. Quote the whole request. |
| Something else is off | Rerun with `-v` to see llama.cpp's logs. |

Linux with Vulkan is the tested setup. macOS is declared in the flake but
untested. Windows is not supported.

---

## 🛠️ How it works

```text
your words ──▶ context (OS, shell, cwd listing)
           ──▶ local LLM, output constrained to a JSON schema
           ──▶ risk rules (regex, no LLM)
           ──▶ approval prompt
           ──▶ $SHELL -c "<command>"
```

| File | Role |
|---|---|
| [`src/main.rs`](src/main.rs) | CLI and the flow above |
| [`src/context.rs`](src/context.rs) | Gathers OS, shell and directory listing |
| [`src/suggest.rs`](src/suggest.rs) | Prompt, JSON schema, sampler, generation |
| [`src/risk.rs`](src/risk.rs) | Risk patterns and their tests |
| [`src/ui.rs`](src/ui.rs) | Suggestion card and approval prompt |
| [`src/exec.rs`](src/exec.rs) | Runs the approved command |
| [`src/bin/eval.rs`](src/bin/eval.rs) | Eval harness for [`evals/cases.toml`](evals/cases.toml) |

## 🧑‍💻 Development

```sh
nix develop          # toolchain plus native deps
just build           # CLI and eval harness
just test            # tests, rustfmt check, clippy
just eval            # full model sweep (downloads every candidate model)
just eval-one "name=hf://…"   # evaluate one model
just eval-ab  "name=hf://…"   # compare prompt v1 and v2 on one model
```

Design notes, prior-art research and the work log live in [`notes/`](notes).

## 🗺️ Roadmap

- [x] Describe → suggest → approve → run, with `--print` and `--json`
- [x] Risk rules and the type-`run` gate
- [x] Eval suite and model sweep
- [ ] Richer approval card: syntax highlighting, edit, copy, regenerate
- [ ] Pick between several suggestions
- [ ] Refine loop ("not quite, exclude node_modules")
- [ ] `explain` mode and fix-my-last-command mode
- [ ] Shell history context and shell widgets
- [ ] Dry-run preview, voice input, shell completions

## 📜 License

[EUPL-1.2](LICENSE), matching NobodyWho. llama.cpp and llama-cpp-rs are MIT.
NobodyWho's `kokoro` TTS feature, which would pull in GPL espeak-ng, is
disabled.
