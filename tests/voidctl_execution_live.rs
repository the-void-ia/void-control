#![cfg(feature = "serde")]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct BridgeServer {
    child: Child,
    base_url: String,
}

impl Drop for BridgeServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "requires live void-box daemon with production initramfs and ANTHROPIC_API_KEY"]
fn voidctl_execution_swarm_flow_completes_against_live_bridge() {
    let root = temp_root("voidctl-cli-swarm-live");
    let bridge = spawn_bridge(&root);
    let spec = std::env::current_dir()
        .expect("cwd")
        .join("examples/swarm-transform-optimization-3way.yaml");

    let submit = run_voidctl(
        &bridge.base_url,
        &["execution", "submit", spec.to_string_lossy().as_ref()],
    );
    assert!(
        submit.status.success(),
        "submit stderr={}",
        stdout_stderr(&submit)
    );
    let execution_id = field_value(&String::from_utf8_lossy(&submit.stdout), "execution_id")
        .expect("execution id from submit");

    let watch = run_voidctl(&bridge.base_url, &["execution", "watch", &execution_id]);
    assert!(
        watch.status.success(),
        "watch stderr={}",
        stdout_stderr(&watch)
    );

    let result = run_voidctl(&bridge.base_url, &["execution", "result", &execution_id]);
    assert!(
        result.status.success(),
        "result stderr={}",
        stdout_stderr(&result)
    );
    let result_stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        result_stdout.contains("status=Completed"),
        "{result_stdout}"
    );
    assert!(
        result_stdout.contains("winner_candidate_id="),
        "{result_stdout}"
    );
    assert!(result_stdout.contains("runtime_run_id="), "{result_stdout}");

    let runtime = run_voidctl(&bridge.base_url, &["execution", "runtime", &execution_id]);
    assert!(
        runtime.status.success(),
        "runtime stderr={}",
        stdout_stderr(&runtime)
    );
    let runtime_stdout = String::from_utf8_lossy(&runtime.stdout);
    assert!(runtime_stdout.contains("candidate_id="), "{runtime_stdout}");
    assert!(
        runtime_stdout.contains("runtime_run_id="),
        "{runtime_stdout}"
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn voidctl_execution_wraps_raw_runtime_specs_against_live_bridge() {
    let root = temp_root("voidctl-cli-runtime-live");
    let bridge = spawn_bridge(&root);
    let runtime_spec = write_runtime_spec(&root.join("snapshot_pipeline.yaml"));

    let submit = run_voidctl(
        &bridge.base_url,
        &[
            "execution",
            "submit",
            runtime_spec.to_string_lossy().as_ref(),
        ],
    );
    assert!(
        submit.status.success(),
        "submit stderr={}",
        stdout_stderr(&submit)
    );
    let submit_stdout = String::from_utf8_lossy(&submit.stdout);
    assert!(submit_stdout.contains("mode=swarm"), "{submit_stdout}");
    let execution_id =
        field_value(&submit_stdout, "execution_id").expect("execution id from wrapped submit");

    let watch = run_voidctl(&bridge.base_url, &["execution", "watch", &execution_id]);
    assert!(
        watch.status.success(),
        "watch stderr={}",
        stdout_stderr(&watch)
    );

    let inspect = run_voidctl(&bridge.base_url, &["execution", "inspect", &execution_id]);
    assert!(
        inspect.status.success(),
        "inspect stderr={}",
        stdout_stderr(&inspect)
    );
    let inspect_stdout = String::from_utf8_lossy(&inspect.stdout);
    assert!(inspect_stdout.contains("candidate_id="), "{inspect_stdout}");
    assert!(
        inspect_stdout.contains("runtime_run_id="),
        "{inspect_stdout}"
    );

    let runtime = run_voidctl(&bridge.base_url, &["execution", "runtime", &execution_id]);
    assert!(
        runtime.status.success(),
        "runtime stderr={}",
        stdout_stderr(&runtime)
    );
    let runtime_stdout = String::from_utf8_lossy(&runtime.stdout);
    assert!(
        runtime_stdout.contains("runtime_run_id="),
        "{runtime_stdout}"
    );
}

fn spawn_bridge(root: &Path) -> BridgeServer {
    let port = free_port();
    let listen = format!("127.0.0.1:{port}");
    let base_url = format!("http://{listen}");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    fs::create_dir_all(&spec_dir).expect("create spec dir");
    fs::create_dir_all(&execution_dir).expect("create execution dir");

    let child = Command::new(env!("CARGO_BIN_EXE_voidctl"))
        .arg("serve")
        .env("VOID_CONTROL_BRIDGE_LISTEN", &listen)
        .env("VOID_CONTROL_SPEC_DIR", &spec_dir)
        .env("VOID_CONTROL_EXECUTION_DIR", &execution_dir)
        .env(
            "VOID_BOX_BASE_URL",
            std::env::var("VOID_BOX_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:43100".to_string()),
        )
        .spawn()
        .expect("spawn bridge");

    wait_for_health(&base_url);
    BridgeServer { child, base_url }
}

fn wait_for_health(base_url: &str) {
    for _ in 0..40 {
        if let Ok((status, _body)) = http_request(base_url, "GET", "/v1/health", None) {
            if status == 200 {
                return;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!("bridge did not become healthy at {base_url}");
}

fn run_voidctl(base_url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_voidctl"))
        .args(args)
        .env("VOID_CONTROL_BRIDGE_BASE_URL", base_url)
        .output()
        .expect("run voidctl")
}

fn http_request(
    base_url: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let stripped = base_url
        .strip_prefix("http://")
        .ok_or_else(|| format!("invalid base url {base_url}"))?;
    let (host, port) = match stripped.split_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>()
                .map_err(|_| format!("invalid port in {base_url}"))?,
        ),
        None => (stripped, 80),
    };
    let mut stream =
        TcpStream::connect((host, port)).map_err(|e| format!("connect failed: {e}"))?;
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
        return Err("invalid response".to_string());
    };
    let Some(status) = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    else {
        return Err("invalid status line".to_string());
    };
    let status = status
        .parse::<u16>()
        .map_err(|_| "invalid status code".to_string())?;
    Ok((status, body.to_string()))
}

fn field_value(output: &str, field: &str) -> Option<String> {
    for line in output.lines() {
        for token in line.split_whitespace() {
            let prefix = format!("{field}=");
            let Some(value) = token.strip_prefix(&prefix) else {
                continue;
            };
            return Some(value.to_string());
        }
    }
    None
}

fn stdout_stderr(output: &Output) -> String {
    format!(
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn write_runtime_spec(path: &Path) -> PathBuf {
    fs::write(
        path,
        r#"api_version: v1
kind: workflow
name: snapshot-pipeline

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: analyze
      run:
        program: sh
        args:
          - -lc
          - |
            cat > result.json <<'JSON'
            {"status":"success","summary":"ok","metrics":{"latency_p99_ms":42},"artifacts":[]}
            JSON
  output_step: analyze
"#,
    )
    .expect("write runtime spec");
    path.to_path_buf()
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().expect("listener addr").port();
    drop(listener);
    port
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
