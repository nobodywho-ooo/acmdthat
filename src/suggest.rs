//! Suggestion generation: schema-grammar-constrained output from NobodyWho.

use crate::context::EnvContext;
use nobodywho::chat::ChatHandle;
use nobodywho::miette;
use nobodywho::sampler::{GrammarStep, SampleStep, SamplerBuilder, SamplerConfig, ShiftStep};
use serde::{Deserialize, Serialize};

/// Which system prompt to render. V1 is the initial rule-list prompt; V2 adds
/// few-shot examples, an ambiguity policy and quoting discipline. The eval
/// harness A/Bs these; the CLI defaults to the latest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptVersion {
    V1,
    V2,
}

impl PromptVersion {
    pub fn parse(s: &str) -> miette::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "v1" => Ok(Self::V1),
            "v2" => Ok(Self::V2),
            other => Err(miette::miette!(
                "unknown prompt version {other:?} (expected v1 or v2)"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }
}

pub fn system_prompt(ctx: &EnvContext, version: PromptVersion) -> String {
    match version {
        PromptVersion::V1 => system_prompt_v1(ctx),
        PromptVersion::V2 => system_prompt_v2(ctx),
    }
}

/// JSON schema the model's output is grammar-constrained to (llguidance).
///
/// `maxItems: 1` for M1: exactly one command, no picker yet. M2 widens this to
/// 1–3 with a picker UI; the grammar keeps the same shape either way.
pub const SUGGESTION_SCHEMA: &str = r##"{
    "type": "object",
    "properties": {
        "commands": {
            "type": "array",
            "minItems": 1,
            "maxItems": 1,
            "items": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "explanation": { "type": "string" }
                },
                "required": ["command", "explanation"],
                "additionalProperties": false
            }
        }
    },
    "required": ["commands"],
    "additionalProperties": false
}"##;

/// What the model returns: guaranteed parseable by the grammar constraint.
#[derive(Debug, Serialize, Deserialize)]
pub struct Suggestion {
    pub commands: Vec<CommandSuggestion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandSuggestion {
    pub command: String,
    pub explanation: String,
}

/// Grammar + sampling chain: JSON-schema constraint first (grammar masks before
/// shifts truncate), then default top-k/top-p, low temperature, dist sample.
pub fn sampler() -> SamplerConfig {
    SamplerBuilder::new()
        .shift(ShiftStep::TopK { top_k: 20 })
        .shift(ShiftStep::TopP {
            top_p: 0.95,
            min_keep: 1,
        })
        .shift(ShiftStep::Temperature { temperature: 0.2 })
        .add_grammar(GrammarStep::JsonSchema(SUGGESTION_SCHEMA.to_string()))
        .sample(SampleStep::Dist)
}

pub fn system_prompt_v1(ctx: &EnvContext) -> String {
    let entries = if ctx.entries.is_empty() {
        "(empty or unreadable)".to_string()
    } else {
        ctx.entries
            .iter()
            .map(|e| format!("  {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "You are the command generator for acmdthat, a terminal assistant. \
Convert the user's request into exactly one {shell} command line to run on {os} \
from the working directory {cwd}.\n\
Rules:\n\
- One single command line, valid {shell}, ready to run as-is: no markdown, no \
backticks, no leading '$'.\n\
- Prefer widely available modern tools (rg, fd, jq, awk, sed, ffmpeg, …) when \
they fit the request.\n\
- Use paths relative to the working directory unless the user asks otherwise.\n\
- Multiple steps may be joined with '&&' (only when a later step depends on an \
earlier one) or ';' otherwise. Never more than one line.\n\
- explanation: one short sentence saying what the command does and its key flags.\n\
\n\
Working directory contains:\n{entries}",
        shell = ctx.shell,
        os = ctx.os,
        cwd = ctx.cwd_display(),
    )
}

/// V2: adds few-shot examples, an ambiguity policy, and quoting discipline —
/// the weaknesses observed in V1 while testing small models (0.6B/0.8B).
pub fn system_prompt_v2(ctx: &EnvContext) -> String {
    let entries = if ctx.entries.is_empty() {
        "(empty or unreadable)".to_string()
    } else {
        ctx.entries
            .iter()
            .map(|e| format!("  {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "You are the command generator inside acmdthat, a terminal assistant. \
Convert the user's request into exactly one {shell} command line to run on {os} \
from the working directory {cwd}. Reply with JSON only.\n\
Rules:\n\
- One single line, valid {shell}, ready to run as-is: no markdown, no \
backticks, no leading '$'.\n\
- Prefer widely available modern tools (rg, fd, jq, awk) when they fit; \
fall back to portable POSIX ones (grep, find) only when needed.\n\
- Quote literal strings with single quotes, especially patterns or text \
containing spaces or non-ASCII characters.\n\
- Prefer paths relative to the working directory.\n\
- Steps that depend on each other join with '&&'; independent steps with ';'. \
One line total.\n\
- If the request is ambiguous, pick the most common interpretation and \
mention the alternative briefly in the explanation.\n\
- explanation: one short sentence on what the command does and its key flags.\n\
\n\
Examples:\n\
user: \"show the 10 biggest files under the current directory\"\n\
assistant: {{\"commands\":[{{\"command\":\"du -ah . | sort -rh | head -n 10\",\"explanation\":\"Sizes of everything under ., human-readable, sorted descending, top 10.\"}}]}}\n\
user: \"list which files in this repo contain TODO, ignoring case, unique filenames only\"\n\
assistant: {{\"commands\":[{{\"command\":\"rg -i -l 'TODO' . | sort -u\",\"explanation\":\"ripgrep case-insensitively lists files (-l) matching TODO, deduplicated.\"}}]}}\n\
user: \"extract archive.zip into a directory named out\"\n\
assistant: {{\"commands\":[{{\"command\":\"mkdir -p out && unzip archive.zip -d out\",\"explanation\":\"Creates out if missing, then extracts the archive into it.\"}}]}}\n\
\n\
Working directory contains:\n{entries}",
        shell = ctx.shell,
        os = ctx.os,
        cwd = ctx.cwd_display(),
    )
}

/// Qwen family models ship with thinking mode on by default (slow for this
/// task); other families (Gemma) don't have the template variable. Only set
/// `enable_thinking=false` when the model source looks like a Qwen.
pub fn is_qwen(source: &str) -> bool {
    source.to_ascii_lowercase().contains("qwen")
}

/// Stream a suggestion for `query`, reporting each token piece to `on_token`
/// with the running token count. Returns the raw model output (a JSON object,
/// by construction of the grammar).
pub fn generate(
    chat: &ChatHandle,
    query: &str,
    mut on_token: impl FnMut(&str, usize),
) -> miette::Result<String> {
    let mut stream = chat.ask(query)?;
    let mut count = 0usize;
    while let Some(token) = stream.next_token()? {
        count += 1;
        on_token(&token, count);
    }
    Ok(stream.completed()?)
}

impl Suggestion {
    /// Parse from raw model output. Only fails if the grammar constraint was
    /// somehow violated, which is a bug (or a nobodywho/llguidance bug).
    pub fn parse(raw: &str) -> miette::Result<Self> {
        serde_json::from_str(raw.trim()).map_err(|e| {
            miette::miette!(
                "model produced unparseable JSON ({e}) — impossible under the grammar \
                 constraint, please report this bug"
            )
        })
    }

    /// The command to run: the first (and in M1, only) suggested command.
    pub fn command(&self) -> &str {
        &self
            .commands
            .first()
            .expect("grammar guarantees at least one command")
            .command
    }
}
