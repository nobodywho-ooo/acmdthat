//! acmdthat-eval — run the `evals/cases.toml` suite across models and prompts.
//!
//! Generates one suggestion per case per model (grammar-constrained, no
//! execution), scores against the reference answer, and prints per-case
//! results plus a cross-model summary. Everything is printed as it happens,
//! so a killed run keeps its partial output.
//!
//! ```sh
//! cargo run --bin eval -- --model qwen2b=hf://… --model gemma-e2b=hf://… [--prompt v1|v2]
//! ```

use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Instant;

use acmdthat::context;
use acmdthat::risk;
use acmdthat::suggest::{self, PromptVersion, Suggestion};
use clap::Parser;
use nobodywho::chat::ChatBuilder;
use nobodywho::{llm, miette};
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(
    name = "acmdthat-eval",
    version,
    about = "Run the evals/cases.toml suite across models and prompt versions"
)]
struct Args {
    /// Model as NAME=HF_REF or PATH (repeatable). Bare refs get a derived name.
    #[arg(short = 'M', long = "model", value_name = "NAME=REF", required = true)]
    models: Vec<String>,

    /// System prompt version to evaluate.
    #[arg(short, long, default_value = "v2")]
    prompt: String,

    /// Evaluate only these case ids (repeatable).
    #[arg(long = "case", value_name = "ID")]
    only: Vec<String>,

    /// Case file (default evals/cases.toml).
    #[arg(long, default_value = "evals/cases.toml")]
    cases_file: String,

    /// Force CPU inference.
    #[arg(long)]
    cpu: bool,

    /// Context size.
    #[arg(long, default_value = "4096")]
    ctx: u32,

    /// Append one JSON record per case to this file.
    #[arg(long)]
    jsonl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CaseFile {
    #[serde(rename = "case")]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    prompt: String,
    expected: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug)]
struct CaseResult {
    case: Case,
    latency_s: f64,
    suggestion: Option<Suggestion>,
    error: Option<String>,
}

fn main() -> miette::Result<()> {
    let args = Args::parse();
    let prompt_version = PromptVersion::parse(&args.prompt)?;

    let cases: Vec<Case> = {
        let raw = std::fs::read_to_string(&args.cases_file)
            .map_err(|e| miette::miette!("cannot read {}: {e}", args.cases_file))?;
        let file: CaseFile = toml::from_str(&raw)
            .map_err(|e| miette::miette!("cannot parse {}: {e}", args.cases_file))?;
        if args.only.is_empty() {
            file.cases
        } else {
            file.cases
                .into_iter()
                .filter(|c| args.only.contains(&c.id))
                .collect()
        }
    };
    eprintln!(
        "eval: {} cases, prompt {}, interactive stdout: {}",
        cases.len(),
        prompt_version.as_str(),
        std::io::stdout().is_terminal()
    );

    let ctx = context::gather();
    let use_gpu = !args.cpu && llm::has_gpu_backend();

    let models = parse_models(&args.models)?;
    let mut jsonl = match &args.jsonl {
        Some(path) => {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| miette::miette!("cannot open {path}: {e}"))?;
            Some(f)
        }
        None => None,
    };

    let mut summaries: Vec<(String, Summary)> = vec![];
    for (name, model_ref) in &models {
        eprintln!("== model {name}: loading {model_ref}");
        let t = Instant::now();
        let model = llm::get_model(model_ref, use_gpu, None, None, None)?;
        let model_source = model.source().to_string();
        eprintln!(
            "   loaded in {:.1}s ({})",
            t.elapsed().as_secs_f32(),
            model_source
        );

        let mut chat_builder = ChatBuilder::new(Arc::new(model))
            .with_context_size(args.ctx)
            .with_system_prompt(Some(suggest::system_prompt(&ctx, prompt_version)))
            .with_sampler(suggest::sampler());
        if suggest::is_qwen(&model_source) {
            chat_builder =
                chat_builder.with_template_variable("enable_thinking".to_string(), false);
        }
        let chat = chat_builder.build()?;

        let mut results: Vec<CaseResult> = vec![];
        for case in &cases {
            eprint!(
                "   [{:>3}/{:>3}] {}: generating… ",
                results.len() + 1,
                cases.len(),
                case.id
            );
            let res = run_case(&chat, case);
            eprintln!("{:.1}s", res.latency_s);
            print_case(&res);
            if let Some(f) = jsonl.as_mut() {
                write_jsonl(f, name, prompt_version, &res)?;
            }
            results.push(res);
        }
        let summary = summarize(&results);
        print_summary(name, &summary);
        summaries.push((name.clone(), summary));
    }

    print_cross_table(&summaries);
    Ok(())
}

fn parse_models(specs: &[String]) -> miette::Result<Vec<(String, String)>> {
    specs
        .iter()
        .map(|spec| match spec.split_once('=') {
            Some((name, ref_)) => Ok((name.trim().to_string(), ref_.trim().to_string())),
            None => {
                let name = spec
                    .rsplit('/')
                    .next()
                    .unwrap_or(spec)
                    .trim_end_matches(".gguf")
                    .to_string();
                Ok((name, spec.clone()))
            }
        })
        .collect()
}

fn run_case(chat: &nobodywho::chat::ChatHandle, case: &Case) -> CaseResult {
    // Fresh history per case: same system prompt, no cross-case leakage.
    if let Err(e) = chat.reset_history() {
        return CaseResult {
            case: clone_case(case),
            latency_s: 0.0,
            suggestion: None,
            error: Some(format!("reset_history: {e}")),
        };
    }
    let t = Instant::now();
    let gen = suggest::generate(chat, &case.prompt, |_, _| {});
    let latency_s = t.elapsed().as_secs_f64();
    match gen {
        Ok(raw) => match Suggestion::parse(&raw) {
            Ok(s) => CaseResult {
                case: clone_case(case),
                latency_s,
                suggestion: Some(s),
                error: None,
            },
            Err(e) => CaseResult {
                case: clone_case(case),
                latency_s,
                suggestion: None,
                error: Some(format!("parse: {e}")),
            },
        },
        Err(e) => CaseResult {
            case: clone_case(case),
            latency_s,
            suggestion: None,
            error: Some(format!("generate: {e}")),
        },
    }
}

// Case isn't Clone because of the note field; clone by hand.
fn clone_case(c: &Case) -> Case {
    Case {
        id: c.id.clone(),
        prompt: c.prompt.clone(),
        expected: c.expected.clone(),
        note: c.note.clone(),
    }
}

// ---- scoring ----

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn program(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

fn flag_tokens(s: &str) -> Vec<String> {
    s.split_whitespace()
        .filter(|t| t.starts_with('-') && t.len() > 1)
        .map(str::to_string)
        .collect()
}

fn score(result: &CaseResult) -> Scores {
    let Some(sug) = result.suggestion.as_ref().and_then(|s| s.commands.first()) else {
        return Scores::default();
    };
    let got = &sug.command;
    let expected = &result.case.expected;
    let expected_flags = flag_tokens(expected);
    let got_flags = flag_tokens(got);
    let hits = expected_flags
        .iter()
        .filter(|f| got_flags.contains(f))
        .count();
    let precision = if got_flags.is_empty() {
        if expected_flags.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        hits as f64 / got_flags.len() as f64
    };
    let recall = if expected_flags.is_empty() {
        1.0
    } else {
        hits as f64 / expected_flags.len() as f64
    };
    Scores {
        exact: normalize(got) == normalize(expected),
        program: program(got) == program(expected),
        flags_precision: precision,
        flags_recall: recall,
    }
}

#[derive(Default)]
struct Scores {
    exact: bool,
    program: bool,
    flags_precision: f64,
    flags_recall: f64,
}

#[derive(Debug)]
struct Summary {
    n: usize,
    exact: usize,
    program: usize,
    errors: usize,
    flags_precision: f64,
    flags_recall: f64,
    mean_s: f64,
    median_s: f64,
}

fn summarize(results: &[CaseResult]) -> Summary {
    let mut latencies: Vec<f64> = results.iter().map(|r| r.latency_s).collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = results.len();
    let scored: Vec<Scores> = results.iter().map(score).collect();
    Summary {
        n,
        exact: scored.iter().filter(|s| s.exact).count(),
        program: scored.iter().filter(|s| s.program).count(),
        errors: results.iter().filter(|r| r.error.is_some()).count(),
        flags_precision: scored.iter().map(|s| s.flags_precision).sum::<f64>() / n.max(1) as f64,
        flags_recall: scored.iter().map(|s| s.flags_recall).sum::<f64>() / n.max(1) as f64,
        mean_s: latencies.iter().sum::<f64>() / n.max(1) as f64,
        median_s: *latencies.get(n / 2).unwrap_or(&0.0),
    }
}

// ---- reporting ----

fn print_case(result: &CaseResult) {
    let s = score(result);
    let exact = if s.exact { "✓" } else { "✗" };
    let prog = if s.program { "✓" } else { "✗" };
    match &result.suggestion {
        Some(sug) => {
            let cmd = &sug
                .commands
                .first()
                .expect("grammar guarantees one")
                .command;
            let risk_label = risk::assess(cmd).risk.label();
            println!(
                "       {id:<14} exact:{exact} prog:{prog} flags:{:.2}/{:.2} risk:{risk_label}",
                s.flags_precision,
                s.flags_recall,
                id = result.case.id,
            );
            println!("         got:      {cmd}");
            if !s.exact {
                println!("         expected: {}", result.case.expected);
            }
        }
        None => {
            println!(
                "       {id:<14} ERROR: {}",
                result.error.clone().unwrap_or_default(),
                id = result.case.id
            );
        }
    }
}

fn print_summary(name: &str, s: &Summary) {
    println!(
        "\n-- {name}: exact {}/{} ({:.0}%) program {}/{} ({:.0}%) errors {} flags P{:.2} R{:.2} mean {:.1}s median {:.1}s\n",
        s.exact,
        s.n,
        100.0 * s.exact as f64 / s.n.max(1) as f64,
        s.program,
        s.n,
        100.0 * s.program as f64 / s.n.max(1) as f64,
        s.errors,
        s.flags_precision,
        s.flags_recall,
        s.mean_s,
        s.median_s,
    );
}

fn print_cross_table(summaries: &[(String, Summary)]) {
    println!("\n=== cross-model summary ===");
    println!(
        "{:<22} {:>7} {:>8} {:>8} {:>10} {:>8} {:>8}",
        "model", "exact", "program", "errors", "flags P/R", "mean", "median"
    );
    for (name, s) in summaries {
        println!(
            "{:<22} {:>4}/{:<2} {:>4}/{:<3} {:>8} {:>5.2}/{:<4.2} {:>6.1}s {:>6.1}s",
            name,
            s.exact,
            s.n,
            s.program,
            s.n,
            s.errors,
            s.flags_precision,
            s.flags_recall,
            s.mean_s,
            s.median_s,
        );
    }
}

fn write_jsonl(
    f: &mut std::fs::File,
    model: &str,
    prompt: PromptVersion,
    result: &CaseResult,
) -> miette::Result<()> {
    use std::io::Write;
    let s = score(result);
    let mut obj = serde_json::Map::new();
    obj.insert("model".into(), model.into());
    obj.insert("prompt".into(), prompt.as_str().into());
    obj.insert("case".into(), result.case.id.clone().into());
    obj.insert("prompt_text".into(), result.case.prompt.clone().into());
    obj.insert("latency_s".into(), result.latency_s.into());
    obj.insert(
        "suggestion".into(),
        result
            .suggestion
            .as_ref()
            .and_then(|s| s.commands.first())
            .map(|c| c.command.clone())
            .unwrap_or_default()
            .into(),
    );
    obj.insert(
        "explanation".into(),
        result
            .suggestion
            .as_ref()
            .and_then(|s| s.commands.first())
            .map(|c| c.explanation.clone())
            .unwrap_or_default()
            .into(),
    );
    obj.insert("expected".into(), result.case.expected.clone().into());
    obj.insert("exact".into(), s.exact.into());
    obj.insert("program".into(), s.program.into());
    obj.insert("flags_precision".into(), s.flags_precision.into());
    obj.insert("flags_recall".into(), s.flags_recall.into());
    if let Some(err) = &result.error {
        obj.insert("error".into(), err.clone().into());
    }
    let line = serde_json::to_string(&obj).map_err(|e| miette::miette!("jsonl: {e}"))?;
    writeln!(f, "{line}").map_err(|e| miette::miette!("jsonl write: {e}"))?;
    Ok(())
}
