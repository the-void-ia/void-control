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

    let mut args = env::args().skip(1);
    if let Some(cmd) = args.next() {
        if cmd == "serve" {
            return void_control::bridge::run_bridge();
        }
        if cmd == "help" || cmd == "--help" || cmd == "-h" {
            println!("voidctl commands:");
            println!("  voidctl                 # interactive terminal console");
            println!("  voidctl serve           # start launch bridge (:43210 by default)");
            println!("  voidctl help            # show this help");
            return Ok(());
        }
        return Err(format!(
            "unknown command '{}'. supported: serve, help",
            cmd
        ));
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
                "/run", "/status", "/events", "/logs", "/cancel", "/list", "/watch", "/resume",
                "/help", "/exit",
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
                    if tokens.iter().any(|t| *t == "--policy") {
                        options.extend(["fast", "balanced", "safe"]);
                    }
                }
                "/events" => options.push("--from"),
                "/logs" => options.push("--follow"),
                "/cancel" => options.push("--reason"),
                "/list" => {
                    options.push("--state");
                    if tokens.iter().any(|t| *t == "--state") {
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
                    .ok_or_else(|| "usage: /run <spec_file> [--run-id <id>] [--policy <preset|json>]".to_string())?
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
        let serialized =
            serde_json::to_string_pretty(session).map_err(|e| format!("serialize session failed: {e}"))?;
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
                        session.last_seen_event_id_by_run.insert(
                            run_id,
                            stopped.terminal_event_id,
                        );
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
        }

        if let Err(e) = save_session(&session_file, &session) {
            eprintln!("warn: {e}");
        }
    }

    Ok(())
}
