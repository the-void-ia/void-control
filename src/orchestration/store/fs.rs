use std::fs;
use std::io;
use std::path::PathBuf;
#[cfg(feature = "serde")]
use std::path::{Component, Path};

use super::ExecutionStore;
use crate::orchestration::events::{ControlEventEnvelope, ControlEventType};
#[cfg(feature = "serde")]
use crate::orchestration::spec::ExecutionSpec;
use crate::orchestration::types::{
    CandidateStatus, Execution, ExecutionAccumulator, ExecutionCandidate, ExecutionSnapshot,
    ExecutionStatus,
};
#[cfg(feature = "serde")]
use crate::orchestration::types::{
    CommunicationIntent, InboxSnapshot, RoutedMessage,
};

#[cfg(not(feature = "serde"))]
mod serde_json {
    use std::collections::BTreeMap;
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct Error(String);

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl std::error::Error for Error {}

    pub trait LocalJson: Sized {
        fn to_json_string(&self) -> Result<String, Error>;
        fn from_json_str(value: &str) -> Result<Self, Error>;
    }

    pub fn to_string<T: LocalJson + ?Sized>(value: &T) -> Result<String, Error> {
        value.to_json_string()
    }

    pub fn from_str<T: LocalJson>(value: &str) -> Result<T, Error> {
        T::from_json_str(value)
    }

    impl LocalJson for BTreeMap<String, String> {
        fn to_json_string(&self) -> Result<String, Error> {
            Ok(encode_map_string(self))
        }

        fn from_json_str(value: &str) -> Result<Self, Error> {
            decode_map_string(value)
        }
    }

    impl LocalJson for BTreeMap<String, f64> {
        fn to_json_string(&self) -> Result<String, Error> {
            Ok(encode_map_f64(self))
        }

        fn from_json_str(value: &str) -> Result<Self, Error> {
            decode_map_f64(value)
        }
    }

    impl LocalJson for Vec<String> {
        fn to_json_string(&self) -> Result<String, Error> {
            Ok(encode_list(self))
        }

        fn from_json_str(value: &str) -> Result<Self, Error> {
            decode_list(value)
        }
    }

    fn encode_map_string(value: &BTreeMap<String, String>) -> String {
        value
            .iter()
            .map(|(key, value)| format!("{}={}", escape(key), escape(value)))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn encode_map_f64(value: &BTreeMap<String, f64>) -> String {
        value
            .iter()
            .map(|(key, value)| format!("{}={}", escape(key), value))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn encode_list(value: &[String]) -> String {
        value.iter().map(|item| escape(item)).collect::<Vec<_>>().join(";")
    }

    fn decode_map_string(value: &str) -> Result<BTreeMap<String, String>, Error> {
        let mut map = BTreeMap::new();
        if value.is_empty() {
            return Ok(map);
        }
        for pair in split_escaped(value, ';') {
            let Some((key, value)) = split_once_escaped(&pair, '=') else {
                return Err(Error(format!("invalid encoded map entry '{pair}'")));
            };
            map.insert(unescape(&key)?, unescape(&value)?);
        }
        Ok(map)
    }

    fn decode_map_f64(value: &str) -> Result<BTreeMap<String, f64>, Error> {
        let mut map = BTreeMap::new();
        if value.is_empty() {
            return Ok(map);
        }
        for pair in split_escaped(value, ';') {
            let Some((key, value)) = split_once_escaped(&pair, '=') else {
                return Err(Error(format!("invalid encoded map entry '{pair}'")));
            };
            let parsed = unescape(&value)?
                .parse::<f64>()
                .map_err(|err| Error(err.to_string()))?;
            map.insert(unescape(&key)?, parsed);
        }
        Ok(map)
    }

    fn decode_list(value: &str) -> Result<Vec<String>, Error> {
        if value.is_empty() {
            return Ok(Vec::new());
        }
        split_escaped(value, ';')
            .into_iter()
            .map(|item| unescape(&item))
            .collect()
    }

    fn split_escaped(value: &str, separator: char) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut escaped = false;
        for ch in value.chars() {
            if escaped {
                current.push('\\');
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == separator {
                parts.push(current);
                current = String::new();
            } else {
                current.push(ch);
            }
        }
        if escaped {
            current.push('\\');
        }
        parts.push(current);
        parts
    }

    fn split_once_escaped(value: &str, separator: char) -> Option<(String, String)> {
        let mut left = String::new();
        let mut right = String::new();
        let mut escaped = false;
        let mut seen_separator = false;
        for ch in value.chars() {
            if escaped {
                let target = if seen_separator { &mut right } else { &mut left };
                target.push('\\');
                target.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == separator && !seen_separator {
                seen_separator = true;
            } else if seen_separator {
                right.push(ch);
            } else {
                left.push(ch);
            }
        }
        if seen_separator {
            Some((left, right))
        } else {
            None
        }
    }

    fn escape(value: &str) -> String {
        let mut escaped = String::new();
        for ch in value.chars() {
            match ch {
                '\\' => escaped.push_str("\\\\"),
                ';' => escaped.push_str("\\;"),
                '=' => escaped.push_str("\\="),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                other => escaped.push(other),
            }
        }
        escaped
    }

    fn unescape(value: &str) -> Result<String, Error> {
        let mut output = String::new();
        let mut escaped = false;
        for ch in value.chars() {
            if escaped {
                output.push(match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    ';' => ';',
                    '=' => '=',
                    other => other,
                });
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else {
                output.push(ch);
            }
        }
        if escaped {
            return Err(Error("dangling escape sequence".to_string()));
        }
        Ok(output)
    }
}

#[derive(Debug, Clone)]
pub struct FsExecutionStore {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutionClaim {
    worker_id: String,
    claimed_at_ms: u64,
}

impl FsExecutionStore {
    const CLAIM_TTL_MS: u64 = 30_000;

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn create_execution(&self, execution: &Execution) -> io::Result<()> {
        let dir = self.execution_dir(&execution.execution_id);
        fs::create_dir_all(&dir)?;
        self.save_execution(execution)
    }

    pub fn save_execution(&self, execution: &Execution) -> io::Result<()> {
        let dir = self.execution_dir(&execution.execution_id);
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("execution.txt"),
            format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                execution.execution_id,
                execution.mode,
                execution.goal,
                status_to_str(&execution.status),
                execution.result_best_candidate_id.as_deref().unwrap_or(""),
                execution.completed_iterations,
                execution.failure_counts.total_candidate_failures
            ),
        )
    }

    pub fn claim_execution(&self, execution_id: &str, worker_id: &str) -> io::Result<bool> {
        let dir = self.execution_dir(execution_id);
        fs::create_dir_all(&dir)?;
        let claim_path = dir.join("claim.txt");
        let claim = ExecutionClaim {
            worker_id: worker_id.to_string(),
            claimed_at_ms: now_ms(),
        };
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim_path)
        {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(serialize_claim(&claim).as_bytes())?;
                Ok(true)
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let existing = match self.load_claim_record(execution_id)? {
                    Some(existing) => existing,
                    None => {
                        fs::remove_file(&claim_path)?;
                        return self.claim_execution(execution_id, worker_id);
                    }
                };
                if claim_is_stale(&existing) {
                    fs::remove_file(&claim_path)?;
                    return self.claim_execution(execution_id, worker_id);
                }
                Ok(false)
            }
            Err(err) => Err(err),
        }
    }

    pub fn release_claim(&self, execution_id: &str) -> io::Result<()> {
        let claim_path = self.execution_dir(execution_id).join("claim.txt");
        match fs::remove_file(claim_path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub fn refresh_claim(&self, execution_id: &str, worker_id: &str) -> io::Result<()> {
        let claim_path = self.execution_dir(execution_id).join("claim.txt");
        let Some(existing) = self.load_claim_record(execution_id)? else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "claim not found",
            ));
        };
        if existing.worker_id != worker_id {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "claim owned by another worker",
            ));
        }
        fs::write(
            claim_path,
            serialize_claim(&ExecutionClaim {
                worker_id: worker_id.to_string(),
                claimed_at_ms: now_ms(),
            }),
        )
    }

    pub fn load_claim(&self, execution_id: &str) -> io::Result<Option<String>> {
        Ok(self
            .load_claim_record(execution_id)?
            .map(|claim| claim.worker_id))
    }

    fn load_claim_record(&self, execution_id: &str) -> io::Result<Option<ExecutionClaim>> {
        let claim_path = self.execution_dir(execution_id).join("claim.txt");
        match fs::read_to_string(claim_path) {
            Ok(contents) => Ok(parse_claim(&contents)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn append_event(
        &self,
        execution_id: &str,
        event: &ControlEventEnvelope,
    ) -> io::Result<()> {
        let path = self.execution_dir(execution_id).join("events.log");
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let next = format!(
            "{}{}|{}|{}\n",
            existing,
            event.execution_id,
            event.seq,
            event.event_type.as_str()
        );
        fs::write(path, next)
    }

    pub fn save_accumulator(
        &self,
        execution_id: &str,
        accumulator: &ExecutionAccumulator,
    ) -> io::Result<()> {
        let best_candidate_overrides = serde_json::to_string(&accumulator.best_candidate_overrides)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let explored_signatures = serde_json::to_string(&accumulator.explored_signatures)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let message_backlog = serde_json::to_string(&accumulator.message_backlog)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        fs::write(
            self.execution_dir(execution_id).join("accumulator.txt"),
            format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                accumulator.scoring_history_len,
                accumulator.completed_iterations,
                accumulator.best_candidate_id.as_deref().unwrap_or(""),
                best_candidate_overrides,
                accumulator.search_phase.as_deref().unwrap_or(""),
                explored_signatures,
                message_backlog,
            ),
        )
    }

    pub fn save_candidate(&self, candidate: &ExecutionCandidate) -> io::Result<()> {
        let dir = self.execution_dir(&candidate.execution_id).join("candidates");
        fs::create_dir_all(&dir)?;
        let overrides = serde_json::to_string(&candidate.overrides)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let metrics = serde_json::to_string(&candidate.metrics)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        fs::write(
            dir.join(format!("{}-{}.txt", candidate.created_seq, candidate.candidate_id)),
            format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
                candidate.execution_id,
                candidate.candidate_id,
                candidate.created_seq,
                candidate.iteration,
                candidate_status_to_str(&candidate.status),
                candidate.runtime_run_id.as_deref().unwrap_or(""),
                overrides,
                candidate
                    .succeeded
                    .map(|value| if value { "true" } else { "false" })
                    .unwrap_or(""),
                metrics,
            ),
        )
    }

    pub fn load_candidates(&self, execution_id: &str) -> io::Result<Vec<ExecutionCandidate>> {
        let dir = self.execution_dir(execution_id).join("candidates");
        match fs::read_dir(&dir) {
            Ok(entries) => {
                let mut candidates = Vec::new();
                for entry in entries {
                    let entry = entry?;
                    if entry.file_type()?.is_file() {
                        let body = fs::read_to_string(entry.path())?;
                        candidates.push(parse_candidate(body)?);
                    }
                }
                candidates.sort_by_key(|candidate| candidate.created_seq);
                Ok(candidates)
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(err) => Err(err),
        }
    }

    pub fn clear_iteration_candidates(&self, execution_id: &str, iteration: u32) -> io::Result<()> {
        let dir = self.execution_dir(execution_id).join("candidates");
        match fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    if !entry.file_type()?.is_file() {
                        continue;
                    }
                    let body = fs::read_to_string(entry.path())?;
                    let candidate = parse_candidate(body)?;
                    if candidate.iteration == iteration {
                        fs::remove_file(entry.path())?;
                    }
                }
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    #[cfg(feature = "serde")]
    pub fn append_intent(
        &self,
        execution_id: &str,
        intent: &CommunicationIntent,
    ) -> io::Result<()> {
        append_ndjson_record(self.execution_dir(execution_id).join("intents.log"), intent)
    }

    #[cfg(feature = "serde")]
    pub fn load_intents(&self, execution_id: &str) -> io::Result<Vec<CommunicationIntent>> {
        load_ndjson_records(self.execution_dir(execution_id).join("intents.log"))
    }

    #[cfg(feature = "serde")]
    pub fn append_routed_message(
        &self,
        execution_id: &str,
        message: &RoutedMessage,
    ) -> io::Result<()> {
        append_ndjson_record(self.execution_dir(execution_id).join("messages.log"), message)
    }

    #[cfg(feature = "serde")]
    pub fn load_routed_messages(&self, execution_id: &str) -> io::Result<Vec<RoutedMessage>> {
        load_ndjson_records(self.execution_dir(execution_id).join("messages.log"))
    }

    #[cfg(feature = "serde")]
    pub fn save_inbox_snapshot(&self, snapshot: &InboxSnapshot) -> io::Result<()> {
        let path = self.inbox_snapshot_path(
            &snapshot.execution_id,
            snapshot.iteration,
            &snapshot.candidate_id,
        )?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_vec_pretty(snapshot)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        fs::write(path, payload)
    }

    #[cfg(feature = "serde")]
    pub fn load_inbox_snapshot(
        &self,
        execution_id: &str,
        iteration: u32,
        candidate_id: &str,
    ) -> io::Result<InboxSnapshot> {
        let path = self.inbox_snapshot_path(execution_id, iteration, candidate_id)?;
        let body = fs::read(path)?;
        serde_json::from_slice(&body)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
    }

    #[cfg(feature = "serde")]
    pub fn save_spec(&self, execution_id: &str, spec: &ExecutionSpec) -> io::Result<()> {
        let dir = self.execution_dir(execution_id);
        fs::create_dir_all(&dir)?;
        let payload =
            serde_json::to_vec_pretty(spec).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        fs::write(dir.join("spec.json"), payload)
    }

    #[cfg(feature = "serde")]
    pub fn load_spec(&self, execution_id: &str) -> io::Result<ExecutionSpec> {
        let path = self.execution_dir(execution_id).join("spec.json");
        let body = fs::read(path)?;
        serde_json::from_slice(&body)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    pub fn load_execution(&self, execution_id: &str) -> io::Result<ExecutionSnapshot> {
        let dir = self.execution_dir(execution_id);
        let execution = parse_execution(fs::read_to_string(dir.join("execution.txt"))?)?;
        let events = match fs::read_to_string(dir.join("events.log")) {
            Ok(contents) => parse_events(&contents),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(err),
        };
        let accumulator = match fs::read_to_string(dir.join("accumulator.txt")) {
            Ok(contents) => parse_accumulator(&contents)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => ExecutionAccumulator::default(),
            Err(err) => return Err(err),
        };
        let candidates = self.load_candidates(execution_id)?;

        Ok(ExecutionSnapshot {
            execution,
            events,
            accumulator,
            candidates,
        })
    }

    pub fn list_execution_ids(&self) -> io::Result<Vec<String>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                ids.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn execution_dir(&self, execution_id: &str) -> PathBuf {
        self.root.join(execution_id)
    }

    #[cfg(feature = "serde")]
    fn inbox_snapshot_path(
        &self,
        execution_id: &str,
        iteration: u32,
        candidate_id: &str,
    ) -> io::Result<PathBuf> {
        validate_inbox_candidate_id(candidate_id)?;
        Ok(self
            .execution_dir(execution_id)
            .join("inboxes")
            .join(iteration.to_string())
            .join(format!("{}.json", candidate_id)))
    }
}

impl ExecutionStore for FsExecutionStore {
    fn load_execution(&self, execution_id: &str) -> io::Result<ExecutionSnapshot> {
        FsExecutionStore::load_execution(self, execution_id)
    }

    fn list_active_execution_ids(&self) -> io::Result<Vec<String>> {
        let mut ids = Vec::new();
        for execution_id in self.list_execution_ids()? {
            let snapshot = self.load_execution(&execution_id)?;
            if matches!(
                snapshot.execution.status,
                ExecutionStatus::Pending | ExecutionStatus::Running | ExecutionStatus::Paused
            ) {
                ids.push(execution_id);
            }
        }
        Ok(ids)
    }

    fn load_candidates(&self, execution_id: &str) -> io::Result<Vec<ExecutionCandidate>> {
        FsExecutionStore::load_candidates(self, execution_id)
    }
}

#[cfg(feature = "serde")]
fn append_ndjson_record<T: serde::Serialize>(
    path: PathBuf,
    record: &T,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, record)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    use std::io::Write;
    file.write_all(b"\n")
}

#[cfg(feature = "serde")]
fn load_ndjson_records<T: serde::de::DeserializeOwned>(path: PathBuf) -> io::Result<Vec<T>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let mut records = Vec::new();
            let mut lines = contents.lines().peekable();
            while let Some(line) = lines.next() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str(line) {
                    Ok(record) => records.push(record),
                    Err(err) if lines.peek().is_none() => break,
                    Err(err) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            err.to_string(),
                        ))
                    }
                }
            }
            Ok(records)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

fn parse_execution(contents: String) -> io::Result<Execution> {
    let mut lines = contents.lines();
    let execution_id = required_line(&mut lines, "execution_id")?;
    let mode = required_line(&mut lines, "mode")?;
    let goal = required_line(&mut lines, "goal")?;
    let status = required_line(&mut lines, "status")?;
    let result_best_candidate_id = optional_line(&mut lines)
        .filter(|value| !value.is_empty());
    let completed_iterations = optional_line(&mut lines)
        .map(|value| value.parse().map_err(invalid_data))
        .transpose()?
        .unwrap_or(0);
    let total_candidate_failures = optional_line(&mut lines)
        .map(|value| value.parse().map_err(invalid_data))
        .transpose()?
        .unwrap_or(0);
    Ok(Execution {
        execution_id,
        mode,
        goal,
        status: str_to_status(&status)?,
        result_best_candidate_id,
        completed_iterations,
        failure_counts: crate::orchestration::FailureCounts {
            total_candidate_failures,
        },
    })
}

fn parse_events(contents: &str) -> Vec<ControlEventEnvelope> {
    contents
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('|');
            let execution_id = parts.next()?;
            let seq = parts.next()?.parse().ok()?;
            let event_type = ControlEventType::from_str(parts.next()?)?;
            Some(ControlEventEnvelope::new(execution_id, seq, event_type))
        })
        .collect()
}

fn parse_accumulator(contents: &str) -> io::Result<ExecutionAccumulator> {
    let mut lines = contents.lines();
    let scoring_history_len = required_line(&mut lines, "scoring_history_len")?
        .parse()
        .map_err(invalid_data)?;
    let completed_iterations = required_line(&mut lines, "completed_iterations")?
        .parse()
        .map_err(invalid_data)?;
    let best_candidate_id = optional_line(&mut lines).filter(|value| !value.is_empty());
    let best_candidate_overrides = optional_line(&mut lines)
        .filter(|value| !value.is_empty())
        .map(|value| serde_json::from_str(&value).map_err(invalid_data))
        .transpose()?
        .unwrap_or_default();
    let search_phase = optional_line(&mut lines).filter(|value| !value.is_empty());
    let explored_signatures = optional_line(&mut lines)
        .filter(|value| !value.is_empty())
        .map(|value| serde_json::from_str(&value).map_err(invalid_data))
        .transpose()?
        .unwrap_or_default();
    let message_backlog = optional_line(&mut lines)
        .filter(|value| !value.is_empty())
        .map(|value| serde_json::from_str(&value).map_err(invalid_data))
        .transpose()?
        .unwrap_or_default();
    Ok(ExecutionAccumulator {
        scoring_history_len,
        completed_iterations,
        best_candidate_id,
        best_candidate_overrides,
        search_phase,
        explored_signatures,
        message_backlog,
        ..ExecutionAccumulator::default()
    })
}

fn parse_candidate(contents: String) -> io::Result<ExecutionCandidate> {
    let mut lines = contents.lines();
    let execution_id = required_line(&mut lines, "execution_id")?;
    let candidate_id = required_line(&mut lines, "candidate_id")?;
    let created_seq = required_line(&mut lines, "created_seq")?
        .parse()
        .map_err(invalid_data)?;
    let iteration = required_line(&mut lines, "iteration")?
        .parse()
        .map_err(invalid_data)?;
    let status = required_line(&mut lines, "status")?;
    let runtime_run_id = optional_line(&mut lines).filter(|value| !value.is_empty());
    let overrides = optional_line(&mut lines)
        .filter(|value| !value.is_empty())
        .map(|value| serde_json::from_str(&value).map_err(invalid_data))
        .transpose()?
        .unwrap_or_default();
    let succeeded = optional_line(&mut lines)
        .filter(|value| !value.is_empty())
        .map(|value| match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid candidate success flag '{value}'"),
            )),
        })
        .transpose()?;
    let metrics = optional_line(&mut lines)
        .filter(|value| !value.is_empty())
        .map(|value| serde_json::from_str(&value).map_err(invalid_data))
        .transpose()?
        .unwrap_or_default();
    Ok(ExecutionCandidate {
        execution_id,
        candidate_id,
        created_seq,
        iteration,
        status: str_to_candidate_status(&status)?,
        runtime_run_id,
        overrides,
        succeeded,
        metrics,
    })
}

fn required_line<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> io::Result<String> {
    lines
        .next()
        .map(|line| line.to_string())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("missing {name}")))
}

fn optional_line<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Option<String> {
    lines.next().map(|line| line.to_string())
}

fn status_to_str(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Pending => "Pending",
        ExecutionStatus::Running => "Running",
        ExecutionStatus::Paused => "Paused",
        ExecutionStatus::Completed => "Completed",
        ExecutionStatus::Failed => "Failed",
        ExecutionStatus::Canceled => "Canceled",
    }
}

fn candidate_status_to_str(status: &CandidateStatus) -> &'static str {
    match status {
        CandidateStatus::Queued => "Queued",
        CandidateStatus::Running => "Running",
        CandidateStatus::Completed => "Completed",
        CandidateStatus::Failed => "Failed",
        CandidateStatus::Canceled => "Canceled",
    }
}

fn str_to_status(value: &str) -> io::Result<ExecutionStatus> {
    match value {
        "Pending" => Ok(ExecutionStatus::Pending),
        "Running" => Ok(ExecutionStatus::Running),
        "Paused" => Ok(ExecutionStatus::Paused),
        "Completed" => Ok(ExecutionStatus::Completed),
        "Failed" => Ok(ExecutionStatus::Failed),
        "Canceled" => Ok(ExecutionStatus::Canceled),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown execution status '{value}'"),
        )),
    }
}

fn str_to_candidate_status(value: &str) -> io::Result<CandidateStatus> {
    match value {
        "Queued" => Ok(CandidateStatus::Queued),
        "Running" => Ok(CandidateStatus::Running),
        "Completed" => Ok(CandidateStatus::Completed),
        "Failed" => Ok(CandidateStatus::Failed),
        "Canceled" => Ok(CandidateStatus::Canceled),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown candidate status '{value}'"),
        )),
    }
}

fn invalid_data(err: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

#[cfg(feature = "serde")]
fn validate_inbox_candidate_id(candidate_id: &str) -> io::Result<()> {
    if candidate_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "candidate_id cannot be empty",
        ));
    }
    let path = Path::new(candidate_id);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsafe candidate_id '{candidate_id}'"),
        ))
        }
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn serialize_claim(claim: &ExecutionClaim) -> String {
    format!("{}|{}", claim.worker_id, claim.claimed_at_ms)
}

fn parse_claim(contents: &str) -> Option<ExecutionClaim> {
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (worker_id, claimed_at_ms) = trimmed.split_once('|')?;
    Some(ExecutionClaim {
        worker_id: worker_id.to_string(),
        claimed_at_ms: claimed_at_ms.parse().ok()?,
    })
}

fn claim_is_stale(claim: &ExecutionClaim) -> bool {
    now_ms().saturating_sub(claim.claimed_at_ms) > FsExecutionStore::CLAIM_TTL_MS
}
