#[cfg(not(feature = "serde"))]
fn main() {
    eprintln!("voidctl requires --features serde");
    std::process::exit(1);
}

#[cfg(feature = "serde")]
fn main() {
    if let Err(e) = run() {
        eprintln!("fatal: {e}");
        std::process::exit(1);
    }
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExecutionCommand {
    Submit {
        spec: Option<String>,
        stdin: bool,
    },
    DryRun {
        spec: Option<String>,
        stdin: bool,
    },
    Watch {
        execution_id: String,
    },
    Inspect {
        execution_id: String,
    },
    Events {
        execution_id: String,
    },
    Result {
        execution_id: String,
    },
    Runtime {
        execution_id: String,
        candidate_id: Option<String>,
    },
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    Serve,
    Help,
    Interactive,
    Execution(ExecutionCommand),
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone)]
struct BridgeJsonResponse {
    status: u16,
    json: serde_json::Value,
}

#[cfg(feature = "serde")]
fn execution_result_label_for_mode(mode: &str) -> &'static str {
    if mode == "supervision" {
        "approved_worker"
    } else {
        "best_candidate"
    }
}

#[cfg(feature = "serde")]
fn execution_subcommand_candidates() -> &'static [&'static str] {
    &[
        "submit", "dry-run", "watch", "inspect", "events", "result", "runtime",
    ]
}

#[cfg(feature = "serde")]
fn parse_cli_args<I, S>(args: I) -> Result<CliCommand, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let tokens = args
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect::<Vec<_>>();
    let mut iter = tokens.iter().map(String::as_str);
    let Some(head) = iter.next() else {
        return Ok(CliCommand::Interactive);
    };

    match head {
        "serve" => Ok(CliCommand::Serve),
        "help" | "--help" | "-h" => Ok(CliCommand::Help),
        "execution" => {
            let action = iter
                .next()
                .ok_or_else(|| "usage: voidctl execution <submit|dry-run|watch|inspect|events|result|runtime> [args]".to_string())?;
            match action {
                "submit" => {
                    parse_execution_file_or_stdin(&mut iter, "submit").map(|(spec, stdin)| {
                        CliCommand::Execution(ExecutionCommand::Submit { spec, stdin })
                    })
                }
                "dry-run" => {
                    parse_execution_file_or_stdin(&mut iter, "dry-run").map(|(spec, stdin)| {
                        CliCommand::Execution(ExecutionCommand::DryRun { spec, stdin })
                    })
                }
                "watch" => {
                    let execution_id = iter
                        .next()
                        .ok_or_else(|| "usage: voidctl execution watch <execution_id>".to_string())?
                        .to_string();
                    expect_no_more_args(
                        &mut iter,
                        "usage: voidctl execution watch <execution_id>",
                    )?;
                    Ok(CliCommand::Execution(ExecutionCommand::Watch {
                        execution_id,
                    }))
                }
                "inspect" => {
                    let execution_id = iter
                        .next()
                        .ok_or_else(|| {
                            "usage: voidctl execution inspect <execution_id>".to_string()
                        })?
                        .to_string();
                    expect_no_more_args(
                        &mut iter,
                        "usage: voidctl execution inspect <execution_id>",
                    )?;
                    Ok(CliCommand::Execution(ExecutionCommand::Inspect {
                        execution_id,
                    }))
                }
                "events" => {
                    let execution_id = iter
                        .next()
                        .ok_or_else(|| {
                            "usage: voidctl execution events <execution_id>".to_string()
                        })?
                        .to_string();
                    expect_no_more_args(
                        &mut iter,
                        "usage: voidctl execution events <execution_id>",
                    )?;
                    Ok(CliCommand::Execution(ExecutionCommand::Events {
                        execution_id,
                    }))
                }
                "result" => {
                    let execution_id = iter
                        .next()
                        .ok_or_else(|| {
                            "usage: voidctl execution result <execution_id>".to_string()
                        })?
                        .to_string();
                    expect_no_more_args(
                        &mut iter,
                        "usage: voidctl execution result <execution_id>",
                    )?;
                    Ok(CliCommand::Execution(ExecutionCommand::Result {
                        execution_id,
                    }))
                }
                "runtime" => {
                    let execution_id = iter
                        .next()
                        .ok_or_else(|| {
                            "usage: voidctl execution runtime <execution_id> [candidate_id]"
                                .to_string()
                        })?
                        .to_string();
                    let candidate_id = iter.next().map(|s| s.to_string());
                    if iter.next().is_some() {
                        return Err(
                            "usage: voidctl execution runtime <execution_id> [candidate_id]"
                                .to_string(),
                        );
                    }
                    Ok(CliCommand::Execution(ExecutionCommand::Runtime {
                        execution_id,
                        candidate_id,
                    }))
                }
                other => Err(format!(
                    "unknown execution subcommand '{other}'. supported: {}",
                    execution_subcommand_candidates().join(", ")
                )),
            }
        }
        other => Err(format!(
            "unknown command '{other}'. supported: serve, help, execution"
        )),
    }
}

#[cfg(feature = "serde")]
fn parse_execution_file_or_stdin<'a, I>(
    iter: &mut I,
    action: &str,
) -> Result<(Option<String>, bool), String>
where
    I: Iterator<Item = &'a str>,
{
    let mut spec = None;
    let mut stdin = false;
    for token in iter.by_ref() {
        match token {
            "--stdin" => {
                if stdin || spec.is_some() {
                    return Err(format!(
                        "usage: voidctl execution {action} [<spec-path> | --stdin]"
                    ));
                }
                stdin = true;
            }
            other => {
                if stdin {
                    return Err(format!("unexpected extra argument '{other}'"));
                }
                if spec.is_none() {
                    spec = Some(other.to_string());
                } else {
                    return Err(format!("unexpected extra argument '{other}'"));
                }
            }
        }
    }

    if !stdin && spec.is_none() {
        return Err(format!(
            "usage: voidctl execution {action} [<spec-path> | --stdin]"
        ));
    }

    Ok((spec, stdin))
}

#[cfg(feature = "serde")]
fn expect_no_more_args<I>(iter: &mut I, usage: &str) -> Result<(), String>
where
    I: Iterator,
{
    if iter.next().is_some() {
        Err(usage.to_string())
    } else {
        Ok(())
    }
}

#[cfg(feature = "serde")]
fn top_level_help_text() -> &'static str {
    "voidctl commands:
  voidctl                         # interactive terminal console
  voidctl serve                   # start launch bridge (:43210 by default)
  voidctl help                    # show this help
  voidctl execution submit <spec-path>
  voidctl execution submit --stdin
  voidctl execution dry-run <spec-path>
  voidctl execution dry-run --stdin
  voidctl execution watch <execution-id>
  voidctl execution inspect <execution-id>
  voidctl execution events <execution-id>
  voidctl execution result <execution-id>
  voidctl execution runtime <execution-id> [candidate-id]"
}

#[cfg(feature = "serde")]
fn parse_host_port(base_url: &str) -> Result<(String, u16), String> {
    let stripped = base_url
        .strip_prefix("http://")
        .ok_or_else(|| format!("bridge URL must start with http://, got '{base_url}'"))?;
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    match host_port.split_once(':') {
        Some((host, port)) => port
            .parse::<u16>()
            .map(|port| (host.to_string(), port))
            .map_err(|_| format!("invalid port in bridge URL '{base_url}'")),
        None => Ok((host_port.to_string(), 80)),
    }
}

#[cfg(feature = "serde")]
fn bridge_request(
    base_url: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<BridgeJsonResponse, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let (host, port) = parse_host_port(base_url)?;
    let mut stream =
        TcpStream::connect(format!("{host}:{port}")).map_err(|e| format!("connect failed: {e}"))?;
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("request write failed: {e}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("response read failed: {e}"))?;
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return Err("invalid HTTP response".to_string());
    };
    let Some(status_line) = headers.lines().next() else {
        return Err("invalid HTTP response status line".to_string());
    };
    let Some(status) = status_line.split_whitespace().nth(1) else {
        return Err("invalid HTTP response status line".to_string());
    };
    let status = status
        .parse::<u16>()
        .map_err(|_| "invalid HTTP status code".to_string())?;
    let json = serde_json::from_str(body).map_err(|e| format!("invalid JSON response: {e}"))?;
    Ok(BridgeJsonResponse { status, json })
}

#[cfg(feature = "serde")]
fn load_execution_spec_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("read execution spec failed: {e}"))
}

#[cfg(feature = "serde")]
fn load_execution_spec_input(spec: Option<&str>, stdin: bool) -> Result<String, String> {
    use std::io::Read;

    if stdin {
        let mut spec = String::new();
        std::io::stdin()
            .read_to_string(&mut spec)
            .map_err(|e| format!("read stdin failed: {e}"))?;
        if spec.trim().is_empty() {
            return Err("stdin spec is empty".to_string());
        }
        return Ok(spec);
    }

    let Some(spec) = spec else {
        return Err("spec path is required unless --stdin is used".to_string());
    };
    load_execution_spec_file(spec)
}

#[cfg(feature = "serde")]
fn execution_status_is_terminal(status: &str) -> bool {
    match status {
        "Completed" | "Failed" | "Canceled" => true,
        "Pending" | "Running" | "Paused" => false,
        _ => false,
    }
}

#[cfg(feature = "serde")]
fn execution_progress_line(detail: &serde_json::Value) -> String {
    let execution = detail.get("execution").unwrap_or(detail);
    let progress = detail.get("progress").unwrap_or(&serde_json::Value::Null);
    let mode = execution
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("swarm");
    format!(
        "execution_id={} status={} iterations={} queued={} running={} completed={} failed={} {}={}",
        execution
            .get("execution_id")
            .and_then(|value| value.as_str())
            .unwrap_or("-"),
        execution
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown"),
        detail
            .get("result")
            .and_then(|value| value.get("completed_iterations"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("queued_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("running_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("completed_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("failed_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        execution_result_label_for_mode(mode),
        execution
            .get("result_best_candidate_id")
            .and_then(|value| value.as_str())
            .unwrap_or("-")
    )
}

#[cfg(feature = "serde")]
fn print_execution_summary(detail: &serde_json::Value) {
    let execution = detail.get("execution").unwrap_or(detail);
    let result = detail.get("result").unwrap_or(&serde_json::Value::Null);
    let progress = detail.get("progress").unwrap_or(&serde_json::Value::Null);
    let mode = execution
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("-");
    println!(
        "execution_id={} status={} mode={} goal={} iterations={} {}={}",
        execution
            .get("execution_id")
            .and_then(|value| value.as_str())
            .unwrap_or("-"),
        execution
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown"),
        mode,
        execution
            .get("goal")
            .and_then(|value| value.as_str())
            .unwrap_or("-"),
        result
            .get("completed_iterations")
            .or_else(|| execution.get("completed_iterations"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        execution_result_label_for_mode(mode),
        result
            .get("best_candidate_id")
            .or_else(|| execution.get("result_best_candidate_id"))
            .and_then(|value| value.as_str())
            .unwrap_or("-")
    );
    println!(
        "queued={} running={} completed={} failed={} canceled={} last_event={}",
        progress
            .get("queued_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("running_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("completed_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("failed_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("canceled_candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        progress
            .get("last_event")
            .and_then(|value| value.as_str())
            .unwrap_or("-")
    );
}

#[cfg(feature = "serde")]
fn select_runtime_run(
    detail: &serde_json::Value,
    requested_candidate_id: Option<&str>,
) -> Option<(String, String)> {
    let execution = detail.get("execution").unwrap_or(detail);
    let candidates = detail.get("candidates")?.as_array()?;
    if let Some(requested_candidate_id) = requested_candidate_id {
        for candidate in candidates {
            let candidate_id = candidate
                .get("candidate_id")
                .and_then(|value| value.as_str());
            if candidate_id != Some(requested_candidate_id) {
                continue;
            }
            let runtime_run_id = candidate
                .get("runtime_run_id")
                .and_then(|value| value.as_str())?;
            return Some((
                requested_candidate_id.to_string(),
                runtime_run_id.to_string(),
            ));
        }
        return None;
    }

    let best_candidate_id = execution
        .get("result_best_candidate_id")
        .and_then(|value| value.as_str());
    if let Some(best_candidate_id) = best_candidate_id {
        for candidate in candidates {
            let candidate_id = candidate
                .get("candidate_id")
                .and_then(|value| value.as_str());
            if candidate_id != Some(best_candidate_id) {
                continue;
            }
            let runtime_run_id = candidate
                .get("runtime_run_id")
                .and_then(|value| value.as_str())?;
            return Some((best_candidate_id.to_string(), runtime_run_id.to_string()));
        }
    }

    for candidate in candidates {
        let status = candidate
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let runtime_run_id = candidate
            .get("runtime_run_id")
            .and_then(|value| value.as_str());
        let candidate_id = candidate
            .get("candidate_id")
            .and_then(|value| value.as_str());
        let (Some(runtime_run_id), Some(candidate_id)) = (runtime_run_id, candidate_id) else {
            continue;
        };
        match status {
            "Running" | "Completed" => {
                return Some((candidate_id.to_string(), runtime_run_id.to_string()));
            }
            "Queued" | "Failed" | "Canceled" => {}
            _ => {}
        }
    }

    None
}

#[cfg(feature = "serde")]
fn bridge_error_message(response: &BridgeJsonResponse) -> String {
    let message = response
        .json
        .get("message")
        .and_then(|value| value.as_str());
    if let Some(message) = message {
        return message.to_string();
    }

    let errors = response
        .json
        .get("errors")
        .and_then(|value| value.as_array());
    if let Some(errors) = errors {
        let mut messages = Vec::new();
        for error in errors {
            let Some(error) = error.as_str() else {
                continue;
            };
            messages.push(error.to_string());
        }
        if !messages.is_empty() {
            return messages.join("; ");
        }
    }

    format!("bridge request failed with status {}", response.status)
}

#[cfg(feature = "serde")]
fn run() -> Result<(), String> {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use rustyline::completion::{Completer, Pair};
    use rustyline::error::ReadlineError;
    use rustyline::highlight::Highlighter;
    use rustyline::hint::Hinter;
    use rustyline::history::DefaultHistory;
    use rustyline::validate::Validator;
    use rustyline::{Context, Editor, Helper};
    use serde::{Deserialize, Serialize};
    use void_control::contract::{
        ContractError, ContractErrorCode, EventEnvelope, ExecutionPolicy, RunState, StartRequest,
        StopRequest, SubscribeEventsRequest,
    };
    use void_control::runtime::VoidBoxRuntimeClient;

    let args = env::args().skip(1).collect::<Vec<_>>();
    let parsed_cli = parse_cli_args(args.iter().map(String::as_str))?;
    match &parsed_cli {
        CliCommand::Serve => return void_control::bridge::run_bridge(),
        CliCommand::Help => {
            println!("{}", top_level_help_text());
            return Ok(());
        }
        CliCommand::Execution(_) => {}
        CliCommand::Interactive => {}
    }

    #[derive(Debug, Default, Serialize, Deserialize)]
    struct ConsoleSession {
        last_selected_run: Option<String>,
        last_seen_event_id_by_run: BTreeMap<String, String>,
        recent_commands: Vec<String>,
    }

    #[derive(Default)]
    struct VoidCtlHelper;

    impl Helper for VoidCtlHelper {}
    impl Highlighter for VoidCtlHelper {}
    impl Validator for VoidCtlHelper {}
    impl Hinter for VoidCtlHelper {
        type Hint = String;
    }

    impl Completer for VoidCtlHelper {
        type Candidate = Pair;

        fn complete(
            &self,
            line: &str,
            pos: usize,
            _ctx: &Context<'_>,
        ) -> Result<(usize, Vec<Pair>), ReadlineError> {
            let safe_pos = pos.min(line.len());
            let head = &line[..safe_pos];
            let tokens = head.split_whitespace().collect::<Vec<_>>();

            let command_candidates = [
                "/run",
                "/status",
                "/events",
                "/logs",
                "/cancel",
                "/list",
                "/watch",
                "/resume",
                "/execution",
                "/help",
                "/exit",
            ];

            let mut out = Vec::new();

            if tokens.is_empty() || (tokens.len() == 1 && !head.ends_with(' ')) {
                let prefix = tokens.first().copied().unwrap_or("");
                for cmd in command_candidates {
                    if cmd.starts_with(prefix) {
                        out.push(Pair {
                            display: cmd.to_string(),
                            replacement: cmd.to_string(),
                        });
                    }
                }
                return Ok((0, out));
            }

            let cmd = tokens[0];
            let current = if head.ends_with(' ') {
                ""
            } else {
                tokens.last().copied().unwrap_or("")
            };

            let mut options: Vec<&str> = Vec::new();
            match cmd {
                "/run" => {
                    options.extend(["--run-id", "--policy"]);
                    if tokens.contains(&"--policy") {
                        options.extend(["fast", "balanced", "safe"]);
                    }
                }
                "/execution" => options.extend([
                    "create", "dry-run", "list", "status", "pause", "resume", "cancel", "patch",
                ]),
                "/events" => options.push("--from"),
                "/logs" => options.push("--follow"),
                "/cancel" => options.push("--reason"),
                "/list" => {
                    options.push("--state");
                    if tokens.contains(&"--state") {
                        options.extend(["active", "terminal"]);
                    }
                }
                _ => {}
            }

            let start = safe_pos.saturating_sub(current.len());
            for opt in options {
                if opt.starts_with(current) {
                    out.push(Pair {
                        display: opt.to_string(),
                        replacement: opt.to_string(),
                    });
                }
            }
            Ok((start, out))
        }
    }

    #[derive(Debug)]
    enum Command {
        Run {
            spec: String,
            run_id: Option<String>,
            policy: Option<String>,
        },
        Status {
            run_id: String,
        },
        Events {
            run_id: String,
            from_event_id: Option<String>,
        },
        Logs {
            run_id: String,
            follow: bool,
        },
        Cancel {
            run_id: String,
            reason: String,
        },
        List {
            state: Option<String>,
        },
        Watch {
            run_id: String,
        },
        Resume {
            run_id: String,
        },
        ExecutionCreate {
            spec: String,
        },
        ExecutionDryRun {
            spec: String,
        },
        ExecutionList,
        ExecutionStatus {
            execution_id: String,
        },
        ExecutionPause {
            execution_id: String,
        },
        ExecutionResume {
            execution_id: String,
        },
        ExecutionCancel {
            execution_id: String,
        },
        ExecutionPatch {
            execution_id: String,
            max_iterations: Option<u32>,
            max_concurrent_candidates: Option<u32>,
        },
        Help,
        Exit,
        Empty,
    }

    fn default_policy() -> ExecutionPolicy {
        ExecutionPolicy {
            max_parallel_microvms_per_run: 2,
            max_stage_retries: 1,
            stage_timeout_secs: 300,
            cancel_grace_period_secs: 10,
        }
    }

    fn policy_from_preset(name: &str) -> Option<ExecutionPolicy> {
        match name {
            "fast" => Some(ExecutionPolicy {
                max_parallel_microvms_per_run: 4,
                max_stage_retries: 0,
                stage_timeout_secs: 120,
                cancel_grace_period_secs: 5,
            }),
            "balanced" => Some(default_policy()),
            "safe" => Some(ExecutionPolicy {
                max_parallel_microvms_per_run: 1,
                max_stage_retries: 2,
                stage_timeout_secs: 900,
                cancel_grace_period_secs: 20,
            }),
            _ => None,
        }
    }

    fn parse_policy(raw: Option<String>) -> Result<ExecutionPolicy, String> {
        let Some(raw) = raw else {
            return Ok(default_policy());
        };
        if let Some(p) = policy_from_preset(&raw.to_ascii_lowercase()) {
            return Ok(p);
        }
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("invalid policy JSON: {e}"))?;
        let policy = ExecutionPolicy {
            max_parallel_microvms_per_run: value
                .get("max_parallel_microvms_per_run")
                .and_then(|v| v.as_u64())
                .unwrap_or(2) as u32,
            max_stage_retries: value
                .get("max_stage_retries")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u32,
            stage_timeout_secs: value
                .get("stage_timeout_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(300) as u32,
            cancel_grace_period_secs: value
                .get("cancel_grace_period_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as u32,
        };
        Ok(policy)
    }

    fn parse_command(line: &str) -> Result<Command, String> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Command::Empty);
        }
        if !line.starts_with('/') {
            return Err("commands must start with '/'".to_string());
        }
        let mut tokens = line.split_whitespace();
        let head = tokens.next().unwrap_or_default();
        match head {
            "/run" => {
                let spec = tokens
                    .next()
                    .ok_or_else(|| {
                        "usage: /run <spec_file> [--run-id <id>] [--policy <preset|json>]"
                            .to_string()
                    })?
                    .to_string();
                let mut run_id = None;
                let mut policy = None;
                let rest = tokens.collect::<Vec<_>>();
                let mut idx = 0usize;
                while idx < rest.len() {
                    match rest[idx] {
                        "--run-id" => {
                            idx += 1;
                            if idx >= rest.len() {
                                return Err("missing value for --run-id".to_string());
                            }
                            run_id = Some(rest[idx].to_string());
                        }
                        "--policy" => {
                            idx += 1;
                            if idx >= rest.len() {
                                return Err("missing value for --policy".to_string());
                            }
                            policy = Some(rest[idx].to_string());
                        }
                        other => {
                            return Err(format!("unknown /run option '{other}'"));
                        }
                    }
                    idx += 1;
                }
                Ok(Command::Run {
                    spec,
                    run_id,
                    policy,
                })
            }
            "/status" => Ok(Command::Status {
                run_id: tokens
                    .next()
                    .ok_or_else(|| "usage: /status <run_id>".to_string())?
                    .to_string(),
            }),
            "/events" => {
                let run_id = tokens
                    .next()
                    .ok_or_else(|| "usage: /events <run_id> [--from <event_id>]".to_string())?
                    .to_string();
                let mut from_event_id = None;
                let rest = tokens.collect::<Vec<_>>();
                let mut idx = 0usize;
                while idx < rest.len() {
                    match rest[idx] {
                        "--from" => {
                            idx += 1;
                            if idx >= rest.len() {
                                return Err("missing value for --from".to_string());
                            }
                            from_event_id = Some(rest[idx].to_string());
                        }
                        other => return Err(format!("unknown /events option '{other}'")),
                    }
                    idx += 1;
                }
                Ok(Command::Events {
                    run_id,
                    from_event_id,
                })
            }
            "/logs" => {
                let run_id = tokens
                    .next()
                    .ok_or_else(|| "usage: /logs <run_id> [--follow]".to_string())?
                    .to_string();
                let mut follow = false;
                for token in tokens {
                    if token == "--follow" {
                        follow = true;
                    } else {
                        return Err(format!("unknown /logs option '{token}'"));
                    }
                }
                Ok(Command::Logs { run_id, follow })
            }
            "/cancel" => {
                let run_id = tokens
                    .next()
                    .ok_or_else(|| "usage: /cancel <run_id> [--reason <text>]".to_string())?
                    .to_string();
                let reason = if let Some(pos) = line.find("--reason") {
                    line[pos + "--reason".len()..].trim().to_string()
                } else {
                    "user requested".to_string()
                };
                Ok(Command::Cancel { run_id, reason })
            }
            "/list" => {
                let mut state = None;
                let rest = tokens.collect::<Vec<_>>();
                let mut idx = 0usize;
                while idx < rest.len() {
                    match rest[idx] {
                        "--state" => {
                            idx += 1;
                            if idx >= rest.len() {
                                return Err("missing value for --state".to_string());
                            }
                            state = Some(rest[idx].to_string());
                        }
                        other => return Err(format!("unknown /list option '{other}'")),
                    }
                    idx += 1;
                }
                Ok(Command::List { state })
            }
            "/watch" => Ok(Command::Watch {
                run_id: tokens
                    .next()
                    .ok_or_else(|| "usage: /watch <run_id>".to_string())?
                    .to_string(),
            }),
            "/resume" => Ok(Command::Resume {
                run_id: tokens
                    .next()
                    .ok_or_else(|| "usage: /resume <run_id>".to_string())?
                    .to_string(),
            }),
            "/execution" => {
                let action = tokens.next().ok_or_else(|| {
                    "usage: /execution <create|dry-run|list|status> [args]".to_string()
                })?;
                match action {
                    "create" => Ok(Command::ExecutionCreate {
                        spec: tokens
                            .next()
                            .ok_or_else(|| "usage: /execution create <spec_file>".to_string())?
                            .to_string(),
                    }),
                    "dry-run" => Ok(Command::ExecutionDryRun {
                        spec: tokens
                            .next()
                            .ok_or_else(|| "usage: /execution dry-run <spec_file>".to_string())?
                            .to_string(),
                    }),
                    "list" => Ok(Command::ExecutionList),
                    "status" => Ok(Command::ExecutionStatus {
                        execution_id: tokens
                            .next()
                            .ok_or_else(|| "usage: /execution status <execution_id>".to_string())?
                            .to_string(),
                    }),
                    "pause" => Ok(Command::ExecutionPause {
                        execution_id: tokens
                            .next()
                            .ok_or_else(|| "usage: /execution pause <execution_id>".to_string())?
                            .to_string(),
                    }),
                    "resume" => Ok(Command::ExecutionResume {
                        execution_id: tokens
                            .next()
                            .ok_or_else(|| "usage: /execution resume <execution_id>".to_string())?
                            .to_string(),
                    }),
                    "cancel" => Ok(Command::ExecutionCancel {
                        execution_id: tokens
                            .next()
                            .ok_or_else(|| "usage: /execution cancel <execution_id>".to_string())?
                            .to_string(),
                    }),
                    "patch" => {
                        let execution_id = tokens
                            .next()
                            .ok_or_else(|| {
                                "usage: /execution patch <execution_id> [--max-iterations N] [--max-concurrent-candidates N]".to_string()
                            })?
                            .to_string();
                        let rest = tokens.collect::<Vec<_>>();
                        let mut idx = 0usize;
                        let mut max_iterations = None;
                        let mut max_concurrent_candidates = None;
                        while idx < rest.len() {
                            match rest[idx] {
                                "--max-iterations" => {
                                    idx += 1;
                                    if idx >= rest.len() {
                                        return Err(
                                            "missing value for --max-iterations".to_string()
                                        );
                                    }
                                    max_iterations =
                                        Some(rest[idx].parse::<u32>().map_err(|_| {
                                            "invalid integer for --max-iterations".to_string()
                                        })?);
                                }
                                "--max-concurrent-candidates" => {
                                    idx += 1;
                                    if idx >= rest.len() {
                                        return Err(
                                            "missing value for --max-concurrent-candidates"
                                                .to_string(),
                                        );
                                    }
                                    max_concurrent_candidates =
                                        Some(rest[idx].parse::<u32>().map_err(|_| {
                                            "invalid integer for --max-concurrent-candidates"
                                                .to_string()
                                        })?);
                                }
                                other => {
                                    return Err(format!(
                                        "unknown /execution patch option '{other}'"
                                    ));
                                }
                            }
                            idx += 1;
                        }
                        if max_iterations.is_none() && max_concurrent_candidates.is_none() {
                            return Err(
                                "usage: /execution patch <execution_id> [--max-iterations N] [--max-concurrent-candidates N]"
                                    .to_string(),
                            );
                        }
                        Ok(Command::ExecutionPatch {
                            execution_id,
                            max_iterations,
                            max_concurrent_candidates,
                        })
                    }
                    other => Err(format!("unknown /execution action '{other}'")),
                }
            }
            "/help" => Ok(Command::Help),
            "/exit" | "/quit" => Ok(Command::Exit),
            other => Err(format!("unknown command '{other}'")),
        }
    }

    fn help_text() -> &'static str {
        "Commands:
  /run <spec_file> [--run-id <id>] [--policy <preset|json>]
  /status <run_id>
  /events <run_id> [--from <event_id>]
  /logs <run_id> [--follow]
  /cancel <run_id> [--reason <text>]
  /list [--state active|terminal]
  /watch <run_id>
  /resume <run_id>
  /execution create <spec_file>
  /execution dry-run <spec_file>
  /execution list
  /execution status <execution_id>
  /execution pause <execution_id>
  /execution resume <execution_id>
  /execution cancel <execution_id>
  /execution patch <execution_id> [--max-iterations N] [--max-concurrent-candidates N]
  /help
  /exit

Policy presets: fast | balanced | safe"
    }

    fn default_logo() -> &'static str {
        r#"
 _    __      _     __     ______            __             __
| |  / /___  (_)___/ /    / ____/___  ____  / /__________  / /
| | / / __ \/ / __  /    / /   / __ \/ __ \/ __/ ___/ __ \/ /
| |/ / /_/ / / /_/ /    / /___/ /_/ / / / / /_/ /  / /_/ / /
|___/\____/_/\__,_/     \____/\____/_/ /_/\__/_/   \____/_/
"#
    }

    fn load_logo() -> String {
        if let Ok(path) = env::var("VOID_CONTROL_LOGO_PATH") {
            if let Ok(content) = fs::read_to_string(path) {
                return content;
            }
        }
        default_logo().to_string()
    }

    fn state_color(state: RunState) -> &'static str {
        match state {
            RunState::Running | RunState::Starting | RunState::Pending => "\x1b[34m",
            RunState::Succeeded => "\x1b[32m",
            RunState::Failed => "\x1b[31m",
            RunState::Canceled => "\x1b[90m",
        }
    }

    fn reset_color() -> &'static str {
        "\x1b[0m"
    }

    fn print_event(e: &EventEnvelope) {
        println!(
            "[{}][seq={}][{:?}][run={}] {}",
            e.timestamp,
            e.seq,
            e.event_type,
            e.run_id,
            if e.payload.is_empty() {
                String::new()
            } else {
                format!("{:?}", e.payload)
            }
        );
    }

    fn print_event_live(e: &EventEnvelope) {
        print!("\r\x1b[2K");
        print_event(e);
    }

    fn print_contract_error(err: &ContractError) {
        println!(
            "error: code={:?} retryable={} message={}",
            err.code, err.retryable, err.message
        );
        match err.code {
            ContractErrorCode::NotFound => println!("hint: use /list to discover available runs"),
            ContractErrorCode::AlreadyTerminal => {
                println!("hint: run is terminal, use /status or /events")
            }
            ContractErrorCode::InvalidPolicy => {
                println!("hint: use /run ... --policy balanced|fast|safe")
            }
            _ => {}
        }
    }

    fn session_path() -> PathBuf {
        if let Ok(custom) = env::var("VOID_CONTROL_SESSION_FILE") {
            return PathBuf::from(custom);
        }
        if let Ok(home) = env::var("HOME") {
            return Path::new(&home).join(".void-control/session.json");
        }
        PathBuf::from("./.void-control-session.json")
    }

    fn load_session(path: &Path) -> ConsoleSession {
        let Ok(content) = fs::read_to_string(path) else {
            return ConsoleSession::default();
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    fn save_session(path: &Path, session: &ConsoleSession) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create session dir failed: {e}"))?;
        }
        let serialized = serde_json::to_string_pretty(session)
            .map_err(|e| format!("serialize session failed: {e}"))?;
        fs::write(path, serialized).map_err(|e| format!("write session failed: {e}"))
    }

    fn run_id_to_handle(run_id: &str) -> String {
        format!("vb:{run_id}")
    }

    fn generate_run_id() -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("run-{millis}")
    }

    fn print_status_line(inspect: &void_control::contract::RuntimeInspection) {
        let color = state_color(inspect.state);
        println!(
            "{}run_id={} attempt={} state={:?} active_stage_count={} active_microvm_count={}{}",
            color,
            inspect.run_id,
            inspect.attempt_id,
            inspect.state,
            inspect.active_stage_count,
            inspect.active_microvm_count,
            reset_color()
        );
    }

    fn format_status_bar(
        inspect: &void_control::contract::RuntimeInspection,
        last_event_id: Option<&str>,
    ) -> String {
        format!(
            "[run={} attempt={} state={:?} stages={} microvms={} last_event={}]",
            inspect.run_id,
            inspect.attempt_id,
            inspect.state,
            inspect.active_stage_count,
            inspect.active_microvm_count,
            last_event_id.unwrap_or("-")
        )
    }

    fn render_status_bar(bar: &str) {
        print!("\r\x1b[2K{bar}");
        let _ = io::stdout().flush();
    }

    fn stream_run(
        client: &VoidBoxRuntimeClient,
        session: &mut ConsoleSession,
        run_id: &str,
        logs_only: bool,
        show_status: bool,
    ) {
        let handle = run_id_to_handle(run_id);
        println!("streaming run={} (Ctrl+C to stop)", run_id);
        loop {
            let from = session.last_seen_event_id_by_run.get(run_id).cloned();
            match client.subscribe_events(SubscribeEventsRequest {
                handle: handle.clone(),
                from_event_id: from,
            }) {
                Ok(events) => {
                    for event in &events {
                        if logs_only && event.payload.is_empty() {
                            continue;
                        }
                        print_event_live(event);
                    }
                    if let Some(last) = events.last() {
                        session
                            .last_seen_event_id_by_run
                            .insert(run_id.to_string(), last.event_id.clone());
                    }
                }
                Err(err) => {
                    print_contract_error(&err);
                    break;
                }
            }

            match client.inspect(&handle) {
                Ok(inspect) => {
                    if show_status {
                        let last_event = session
                            .last_seen_event_id_by_run
                            .get(run_id)
                            .map(String::as_str);
                        let bar = format_status_bar(&inspect, last_event);
                        render_status_bar(&bar);
                    }
                    if inspect.state.is_terminal() {
                        print!("\r\x1b[2K");
                        print_status_line(&inspect);
                        println!("terminal state reached: {:?}", inspect.state);
                        break;
                    }
                }
                Err(err) => {
                    print_contract_error(&err);
                    break;
                }
            }

            std::thread::sleep(Duration::from_millis(client.poll_interval_ms()));
        }
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }

    let base_url =
        env::var("VOID_BOX_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:43100".to_string());
    let bridge_base_url = env::var("VOID_CONTROL_BRIDGE_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:43210".to_string());

    if let CliCommand::Execution(command) = parsed_cli {
        match command {
            ExecutionCommand::Submit { spec, stdin } => {
                let spec = load_execution_spec_input(spec.as_deref(), stdin)?;
                match bridge_request(&bridge_base_url, "POST", "/v1/executions", Some(&spec)) {
                    Ok(response) => {
                        if response.status >= 400 {
                            return Err(bridge_error_message(&response));
                        }
                        print_execution_summary(&response.json);
                    }
                    Err(err) => return Err(err),
                }
                return Ok(());
            }
            ExecutionCommand::DryRun { spec, stdin } => {
                let spec = load_execution_spec_input(spec.as_deref(), stdin)?;
                match bridge_request(
                    &bridge_base_url,
                    "POST",
                    "/v1/executions/dry-run",
                    Some(&spec),
                ) {
                    Ok(response) => {
                        if response.status >= 400 {
                            let valid = response
                                .json
                                .get("valid")
                                .and_then(|value| value.as_bool())
                                .unwrap_or(false);
                            if valid {
                                return Err(bridge_error_message(&response));
                            }
                            println!("valid=false");
                            let errors = response
                                .json
                                .get("errors")
                                .and_then(|value| value.as_array());
                            let Some(errors) = errors else {
                                return Err(bridge_error_message(&response));
                            };
                            for error in errors {
                                let Some(error) = error.as_str() else {
                                    continue;
                                };
                                println!("error={error}");
                            }
                            return Err("dry-run validation failed".to_string());
                        }
                        let json = response.json;
                        println!(
                            "valid={} candidates_per_iteration={} max_iterations={} max_child_runs={}",
                            json.get("valid").and_then(|value| value.as_bool()).unwrap_or(false),
                            json.get("plan")
                                .and_then(|value| value.get("candidates_per_iteration"))
                                .and_then(|value| value.as_u64())
                                .unwrap_or(0),
                            json.get("plan")
                                .and_then(|value| value.get("max_iterations"))
                                .and_then(|value| value.as_u64())
                                .unwrap_or(0),
                            json.get("plan")
                                .and_then(|value| value.get("max_child_runs"))
                                .and_then(|value| value.as_u64())
                                .unwrap_or(0)
                        );
                    }
                    Err(err) => return Err(err),
                }
                return Ok(());
            }
            ExecutionCommand::Watch { execution_id } => {
                let mut last_line = String::new();
                let mut last_event_seq = 0u64;
                loop {
                    let path = format!("/v1/executions/{execution_id}");
                    match bridge_request(&bridge_base_url, "GET", &path, None) {
                        Ok(response) => {
                            if response.status >= 400 {
                                return Err(bridge_error_message(&response));
                            }
                            let detail = response.json;
                            let execution = detail.get("execution").unwrap_or(&detail);
                            let line = execution_progress_line(&detail);
                            if line != last_line {
                                println!("{line}");
                                last_line = line;
                            }
                            let path = format!("/v1/executions/{execution_id}/events");
                            let response = bridge_request(&bridge_base_url, "GET", &path, None)?;
                            if response.status >= 400 {
                                return Err(bridge_error_message(&response));
                            }
                            let events = response
                                .json
                                .get("events")
                                .and_then(|value| value.as_array());
                            let Some(events) = events else {
                                return Err("execution events response missing events".to_string());
                            };
                            for event in events {
                                let seq = event
                                    .get("seq")
                                    .and_then(|value| value.as_u64())
                                    .unwrap_or(0);
                                if seq <= last_event_seq {
                                    continue;
                                }
                                println!(
                                    "event seq={} type={}",
                                    seq,
                                    event
                                        .get("event_type")
                                        .and_then(|value| value.as_str())
                                        .unwrap_or("-")
                                );
                                last_event_seq = seq;
                            }
                            let status = execution
                                .get("status")
                                .and_then(|value| value.as_str())
                                .unwrap_or("");
                            if execution_status_is_terminal(status) {
                                break;
                            }
                        }
                        Err(err) => return Err(err),
                    }
                    std::thread::sleep(Duration::from_millis(1000));
                }
                return Ok(());
            }
            ExecutionCommand::Inspect { execution_id } => {
                let path = format!("/v1/executions/{execution_id}");
                match bridge_request(&bridge_base_url, "GET", &path, None) {
                    Ok(response) => {
                        if response.status >= 400 {
                            return Err(bridge_error_message(&response));
                        }
                        let detail = response.json;
                        print_execution_summary(&detail);
                        let candidates = detail
                            .get("candidates")
                            .and_then(|value| value.as_array())
                            .cloned()
                            .unwrap_or_default();
                        for candidate in candidates {
                            println!(
                                "candidate_id={} status={} runtime_run_id={} metrics={}",
                                candidate
                                    .get("candidate_id")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("-"),
                                candidate
                                    .get("status")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("unknown"),
                                candidate
                                    .get("runtime_run_id")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("-"),
                                candidate
                                    .get("metrics")
                                    .cloned()
                                    .unwrap_or_else(|| serde_json::json!({}))
                            );
                        }
                    }
                    Err(err) => return Err(err),
                }
                return Ok(());
            }
            ExecutionCommand::Events { execution_id } => {
                let path = format!("/v1/executions/{execution_id}/events");
                match bridge_request(&bridge_base_url, "GET", &path, None) {
                    Ok(response) => {
                        if response.status >= 400 {
                            return Err(bridge_error_message(&response));
                        }
                        let events = response
                            .json
                            .get("events")
                            .and_then(|value| value.as_array())
                            .cloned()
                            .unwrap_or_default();
                        for event in events {
                            println!(
                                "seq={} event_type={}",
                                event
                                    .get("seq")
                                    .and_then(|value| value.as_u64())
                                    .unwrap_or(0),
                                event
                                    .get("event_type")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("-")
                            );
                        }
                    }
                    Err(err) => return Err(err),
                }
                return Ok(());
            }
            ExecutionCommand::Result { execution_id } => {
                let path = format!("/v1/executions/{execution_id}");
                match bridge_request(&bridge_base_url, "GET", &path, None) {
                    Ok(response) => {
                        if response.status >= 400 {
                            return Err(bridge_error_message(&response));
                        }
                        let detail = response.json;
                        print_execution_summary(&detail);
                        if let Some((candidate_id, runtime_run_id)) =
                            select_runtime_run(&detail, None)
                        {
                            println!(
                                "winner_candidate_id={} runtime_run_id={}",
                                candidate_id, runtime_run_id
                            );
                        }
                        let candidates = detail
                            .get("candidates")
                            .and_then(|value| value.as_array())
                            .cloned()
                            .unwrap_or_default();
                        for candidate in candidates {
                            println!(
                                "candidate_id={} status={} succeeded={} metrics={}",
                                candidate
                                    .get("candidate_id")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("-"),
                                candidate
                                    .get("status")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("unknown"),
                                candidate
                                    .get("succeeded")
                                    .and_then(|value| value.as_bool())
                                    .map(|value| value.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                candidate
                                    .get("metrics")
                                    .cloned()
                                    .unwrap_or_else(|| serde_json::json!({}))
                            );
                        }
                    }
                    Err(err) => return Err(err),
                }
                return Ok(());
            }
            ExecutionCommand::Runtime {
                execution_id,
                candidate_id,
            } => {
                let path = format!("/v1/executions/{execution_id}");
                match bridge_request(&bridge_base_url, "GET", &path, None) {
                    Ok(response) => {
                        if response.status >= 400 {
                            return Err(bridge_error_message(&response));
                        }
                        let runtime = select_runtime_run(&response.json, candidate_id.as_deref());
                        let Some((candidate_id, runtime_run_id)) = runtime else {
                            return Err("no runtime run found for execution".to_string());
                        };
                        println!(
                            "execution_id={} candidate_id={} runtime_run_id={}",
                            execution_id, candidate_id, runtime_run_id
                        );
                    }
                    Err(err) => return Err(err),
                }
                return Ok(());
            }
        }
    }

    let client = VoidBoxRuntimeClient::new(base_url.clone(), 250);
    let session_file = session_path();
    let mut session = load_session(&session_file);
    let mut rl = Editor::<VoidCtlHelper, DefaultHistory>::new()
        .map_err(|e| format!("readline init failed: {e}"))?;
    rl.set_helper(Some(VoidCtlHelper));
    for cmd in &session.recent_commands {
        let _ = rl.add_history_entry(cmd.as_str());
    }

    println!("{}", load_logo());
    println!("voidctl connected to {base_url}");
    println!("{}", help_text());

    loop {
        let line = match rl.readline("voidctl> ") {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => return Err(format!("stdin read failed: {e}")),
        };
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            let _ = rl.add_history_entry(trimmed.as_str());
            session.recent_commands.push(trimmed.clone());
            if session.recent_commands.len() > 200 {
                let keep_from = session.recent_commands.len().saturating_sub(200);
                session.recent_commands = session.recent_commands[keep_from..].to_vec();
            }
        }

        let parsed = match parse_command(&trimmed) {
            Ok(cmd) => cmd,
            Err(e) => {
                println!("{e}");
                continue;
            }
        };

        match parsed {
            Command::Empty => continue,
            Command::Help => println!("{}", help_text()),
            Command::Exit => {
                save_session(&session_file, &session)?;
                println!("bye");
                break;
            }
            Command::Run {
                spec,
                run_id,
                policy,
            } => {
                let run_id = run_id.unwrap_or_else(generate_run_id);
                let policy = match parse_policy(policy) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{e}");
                        continue;
                    }
                };
                match client.start(StartRequest {
                    run_id: run_id.clone(),
                    workflow_spec: spec,
                    launch_context: None,
                    policy,
                }) {
                    Ok(started) => {
                        session.last_selected_run = Some(run_id.clone());
                        println!(
                            "started run_id={} handle={} attempt_id={} state={:?}",
                            run_id, started.handle, started.attempt_id, started.state
                        );
                    }
                    Err(err) => print_contract_error(&err),
                }
            }
            Command::Status { run_id } => {
                let handle = run_id_to_handle(&run_id);
                match client.inspect(&handle) {
                    Ok(inspect) => {
                        session.last_selected_run = Some(run_id);
                        print_status_line(&inspect);
                        println!(
                            "started_at={} updated_at={}",
                            inspect.started_at, inspect.updated_at
                        );
                        if let Some(reason) = inspect.terminal_reason {
                            println!("terminal_reason={reason}");
                        }
                        if let Some(code) = inspect.exit_code {
                            println!("exit_code={code}");
                        }
                    }
                    Err(err) => print_contract_error(&err),
                }
            }
            Command::Events {
                run_id,
                from_event_id,
            } => {
                let handle = run_id_to_handle(&run_id);
                match client.subscribe_events(SubscribeEventsRequest {
                    handle,
                    from_event_id,
                }) {
                    Ok(events) => {
                        for event in &events {
                            print_event(event);
                        }
                        if let Some(last) = events.last() {
                            session
                                .last_seen_event_id_by_run
                                .insert(run_id.clone(), last.event_id.clone());
                        }
                        session.last_selected_run = Some(run_id);
                    }
                    Err(err) => print_contract_error(&err),
                }
            }
            Command::Logs { run_id, follow } => {
                let handle = run_id_to_handle(&run_id);
                let from = session.last_seen_event_id_by_run.get(&run_id).cloned();
                match client.subscribe_events(SubscribeEventsRequest {
                    handle,
                    from_event_id: from,
                }) {
                    Ok(events) => {
                        for event in &events {
                            if !event.payload.is_empty() {
                                print_event(event);
                            }
                        }
                        if let Some(last) = events.last() {
                            session
                                .last_seen_event_id_by_run
                                .insert(run_id.clone(), last.event_id.clone());
                        }
                        session.last_selected_run = Some(run_id.clone());
                        if follow {
                            stream_run(&client, &mut session, &run_id, true, true);
                        }
                    }
                    Err(err) => print_contract_error(&err),
                }
            }
            Command::Cancel { run_id, reason } => {
                let handle = run_id_to_handle(&run_id);
                match client.stop(StopRequest { handle, reason }) {
                    Ok(stopped) => {
                        println!(
                            "stopped run_id={} state={:?} terminal_event_id={}",
                            run_id, stopped.state, stopped.terminal_event_id
                        );
                        session.last_selected_run = Some(run_id.clone());
                        session
                            .last_seen_event_id_by_run
                            .insert(run_id, stopped.terminal_event_id);
                    }
                    Err(err) => print_contract_error(&err),
                }
            }
            Command::List { state } => {
                let filter = state.as_deref();
                match client.list_runs(filter) {
                    Ok(runs) => {
                        println!("runs={}", runs.len());
                        for r in runs {
                            let color = state_color(r.state);
                            println!(
                                "{}run_id={} attempt={} state={:?} active_stage_count={} active_microvm_count={}{}",
                                color,
                                r.run_id,
                                r.attempt_id,
                                r.state,
                                r.active_stage_count,
                                r.active_microvm_count,
                                reset_color()
                            );
                        }
                    }
                    Err(err) => print_contract_error(&err),
                }
            }
            Command::Watch { run_id } => {
                session.last_selected_run = Some(run_id.clone());
                stream_run(&client, &mut session, &run_id, false, true);
            }
            Command::Resume { run_id } => {
                session.last_selected_run = Some(run_id.clone());
                stream_run(&client, &mut session, &run_id, false, true);
            }
            Command::ExecutionCreate { spec } => {
                match load_execution_spec_file(&spec).and_then(|spec_text| {
                    bridge_request(&bridge_base_url, "POST", "/v1/executions", Some(&spec_text))
                }) {
                    Ok(response) => print_execution_summary(&response.json),
                    Err(err) => println!("error: {err}"),
                }
            }
            Command::ExecutionDryRun { spec } => {
                match load_execution_spec_file(&spec).and_then(|spec_text| {
                    bridge_request(
                        &bridge_base_url,
                        "POST",
                        "/v1/executions/dry-run",
                        Some(&spec_text),
                    )
                }) {
                    Ok(response) => println!(
                        "valid={} candidates_per_iteration={} max_iterations={} max_child_runs={}",
                        response
                            .json
                            .get("valid")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        response
                            .json
                            .get("plan")
                            .and_then(|v| v.get("candidates_per_iteration"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        response
                            .json
                            .get("plan")
                            .and_then(|v| v.get("max_iterations"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        response
                            .json
                            .get("plan")
                            .and_then(|v| v.get("max_child_runs"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0)
                    ),
                    Err(err) => println!("error: {err}"),
                }
            }
            Command::ExecutionList => {
                match bridge_request(&bridge_base_url, "GET", "/v1/executions", None) {
                    Ok(response) => {
                        let executions = response
                            .json
                            .get("executions")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        if executions.is_empty() {
                            println!("no executions");
                        } else {
                            for execution in executions {
                                print_execution_summary(&execution);
                            }
                        }
                    }
                    Err(err) => println!("error: {err}"),
                }
            }
            Command::ExecutionStatus { execution_id } => match bridge_request(
                &bridge_base_url,
                "GET",
                &format!("/v1/executions/{execution_id}"),
                None,
            ) {
                Ok(response) => print_execution_summary(&response.json),
                Err(err) => println!("error: {err}"),
            },
            Command::ExecutionPause { execution_id } => match bridge_request(
                &bridge_base_url,
                "POST",
                &format!("/v1/executions/{execution_id}/pause"),
                None,
            ) {
                Ok(response) => println!(
                    "execution_id={} status={}",
                    response
                        .json
                        .get("execution_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-"),
                    response
                        .json
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown"),
                ),
                Err(err) => println!("error: {err}"),
            },
            Command::ExecutionResume { execution_id } => match bridge_request(
                &bridge_base_url,
                "POST",
                &format!("/v1/executions/{execution_id}/resume"),
                None,
            ) {
                Ok(response) => println!(
                    "execution_id={} status={}",
                    response
                        .json
                        .get("execution_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-"),
                    response
                        .json
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown"),
                ),
                Err(err) => println!("error: {err}"),
            },
            Command::ExecutionCancel { execution_id } => match bridge_request(
                &bridge_base_url,
                "POST",
                &format!("/v1/executions/{execution_id}/cancel"),
                None,
            ) {
                Ok(response) => println!(
                    "execution_id={} status={}",
                    response
                        .json
                        .get("execution_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-"),
                    response
                        .json
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown"),
                ),
                Err(err) => println!("error: {err}"),
            },
            Command::ExecutionPatch {
                execution_id,
                max_iterations,
                max_concurrent_candidates,
            } => {
                let body = serde_json::json!({
                    "budget": {
                        "max_iterations": max_iterations
                    },
                    "concurrency": {
                        "max_concurrent_candidates": max_concurrent_candidates
                    }
                })
                .to_string();
                match bridge_request(
                    &bridge_base_url,
                    "PATCH",
                    &format!("/v1/executions/{execution_id}/policy"),
                    Some(&body),
                ) {
                    Ok(response) => println!(
                        "execution_id={} max_iterations={} max_concurrent_candidates={}",
                        response
                            .json
                            .get("execution_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-"),
                        response
                            .json
                            .get("max_iterations")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        response
                            .json
                            .get("max_concurrent_candidates")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0)
                    ),
                    Err(err) => println!("error: {err}"),
                }
            }
        }

        if let Err(e) = save_session(&session_file, &session) {
            eprintln!("warn: {e}");
        }
    }

    Ok(())
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn parses_execution_submit_with_spec_path() {
        let command = parse_cli_args(["execution", "submit", "spec.yaml"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Submit {
                spec: Some("spec.yaml".to_string()),
                stdin: false,
            })
        );
    }

    #[test]
    fn parses_execution_submit_from_stdin() {
        let command = parse_cli_args(["execution", "submit", "--stdin"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Submit {
                spec: None,
                stdin: true,
            })
        );
    }

    #[test]
    fn parses_execution_dry_run_with_spec_path() {
        let command = parse_cli_args(["execution", "dry-run", "spec.yaml"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::DryRun {
                spec: Some("spec.yaml".to_string()),
                stdin: false,
            })
        );
    }

    #[test]
    fn parses_execution_dry_run_from_stdin() {
        let command = parse_cli_args(["execution", "dry-run", "--stdin"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::DryRun {
                spec: None,
                stdin: true,
            })
        );
    }

    #[test]
    fn parses_execution_watch() {
        let command = parse_cli_args(["execution", "watch", "exec-1"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Watch {
                execution_id: "exec-1".to_string(),
            })
        );
    }

    #[test]
    fn parses_execution_inspect() {
        let command = parse_cli_args(["execution", "inspect", "exec-1"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Inspect {
                execution_id: "exec-1".to_string(),
            })
        );
    }

    #[test]
    fn parses_execution_events() {
        let command = parse_cli_args(["execution", "events", "exec-1"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Events {
                execution_id: "exec-1".to_string(),
            })
        );
    }

    #[test]
    fn parses_execution_result() {
        let command = parse_cli_args(["execution", "result", "exec-1"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Result {
                execution_id: "exec-1".to_string(),
            })
        );
    }

    #[test]
    fn parses_execution_runtime_with_optional_candidate() {
        let command = parse_cli_args(["execution", "runtime", "exec-1", "cand-2"]).unwrap();
        assert_eq!(
            command,
            CliCommand::Execution(ExecutionCommand::Runtime {
                execution_id: "exec-1".to_string(),
                candidate_id: Some("cand-2".to_string()),
            })
        );
    }

    #[test]
    fn rejects_extra_execution_watch_args() {
        let err = parse_cli_args(["execution", "watch", "exec-1", "extra"]).unwrap_err();
        assert!(err.contains("usage: voidctl execution watch <execution_id>"));
    }

    #[test]
    fn rejects_extra_execution_submit_stdin_args() {
        let err = parse_cli_args(["execution", "submit", "--stdin", "extra"]).unwrap_err();
        assert!(err.contains("unexpected extra argument"));
    }

    #[test]
    fn completes_execution_subcommands() {
        let completions = execution_subcommand_candidates();
        assert!(completions.contains(&"submit"));
        assert!(completions.contains(&"dry-run"));
        assert!(completions.contains(&"watch"));
        assert!(completions.contains(&"inspect"));
        assert!(completions.contains(&"events"));
        assert!(completions.contains(&"result"));
        assert!(completions.contains(&"runtime"));
    }

    #[test]
    fn top_level_help_mentions_execution_commands() {
        let help = top_level_help_text();
        assert!(help.contains("voidctl execution submit <spec-path>"));
        assert!(help.contains("voidctl execution dry-run --stdin"));
        assert!(help.contains("voidctl execution watch <execution-id>"));
        assert!(help.contains("voidctl execution inspect <execution-id>"));
        assert!(help.contains("voidctl execution events <execution-id>"));
        assert!(help.contains("voidctl execution result <execution-id>"));
        assert!(help.contains("voidctl execution runtime <execution-id> [candidate-id]"));
    }

    #[test]
    fn supervision_execution_uses_approved_worker_label() {
        assert_eq!(
            execution_result_label_for_mode("supervision"),
            "approved_worker"
        );
    }

    #[test]
    fn swarm_execution_uses_best_candidate_label() {
        assert_eq!(execution_result_label_for_mode("swarm"), "best_candidate");
    }

    #[test]
    fn parses_host_port_without_explicit_port() {
        assert_eq!(
            parse_host_port("http://127.0.0.1").unwrap(),
            ("127.0.0.1".to_string(), 80)
        );
    }

    #[test]
    fn bridge_error_message_prefers_message_field() {
        let response = BridgeJsonResponse {
            status: 400,
            json: serde_json::json!({
                "message": "bad request"
            }),
        };

        assert_eq!(bridge_error_message(&response), "bad request");
    }

    #[test]
    fn bridge_error_message_joins_errors_array() {
        let response = BridgeJsonResponse {
            status: 400,
            json: serde_json::json!({
                "errors": ["bad one", "bad two"]
            }),
        };

        assert_eq!(bridge_error_message(&response), "bad one; bad two");
    }

    #[test]
    fn execution_progress_line_formats_execution_detail() {
        let detail = serde_json::json!({
            "execution": {
                "execution_id": "exec-1",
                "status": "Running",
                "mode": "supervision",
                "result_best_candidate_id": "candidate-2"
            },
            "result": {
                "completed_iterations": 2
            },
            "progress": {
                "queued_candidate_count": 1,
                "running_candidate_count": 2,
                "completed_candidate_count": 3,
                "failed_candidate_count": 4
            }
        });

        assert_eq!(
            execution_progress_line(&detail),
            "execution_id=exec-1 status=Running iterations=2 queued=1 running=2 completed=3 failed=4 approved_worker=candidate-2"
        );
    }

    #[test]
    fn execution_status_is_terminal_distinguishes_running_from_completed() {
        assert!(!execution_status_is_terminal("Running"));
        assert!(execution_status_is_terminal("Completed"));
    }

    #[test]
    fn select_runtime_run_prefers_requested_candidate() {
        let detail = serde_json::json!({
            "execution": {
                "execution_id": "exec-1",
                "result_best_candidate_id": "candidate-1"
            },
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "status": "Completed",
                    "runtime_run_id": "run-1"
                },
                {
                    "candidate_id": "candidate-2",
                    "status": "Completed",
                    "runtime_run_id": "run-2"
                }
            ]
        });

        assert_eq!(
            select_runtime_run(&detail, Some("candidate-2")),
            Some(("candidate-2".to_string(), "run-2".to_string()))
        );
    }

    #[test]
    fn select_runtime_run_falls_back_to_best_candidate() {
        let detail = serde_json::json!({
            "execution": {
                "execution_id": "exec-1",
                "result_best_candidate_id": "candidate-2"
            },
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "status": "Running",
                    "runtime_run_id": "run-1"
                },
                {
                    "candidate_id": "candidate-2",
                    "status": "Completed",
                    "runtime_run_id": "run-2"
                }
            ]
        });

        assert_eq!(
            select_runtime_run(&detail, None),
            Some(("candidate-2".to_string(), "run-2".to_string()))
        );
    }

    #[test]
    fn select_runtime_run_falls_back_to_running_candidate() {
        let detail = serde_json::json!({
            "execution": {
                "execution_id": "exec-1"
            },
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "status": "Queued",
                    "runtime_run_id": "run-1"
                },
                {
                    "candidate_id": "candidate-2",
                    "status": "Running",
                    "runtime_run_id": "run-2"
                }
            ]
        });

        assert_eq!(
            select_runtime_run(&detail, None),
            Some(("candidate-2".to_string(), "run-2".to_string()))
        );
    }
}
