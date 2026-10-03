//! End-to-end proof of the AskUserQuestion relay: a real Unix socket, the real
//! binary, a real stdin payload. The unit tests in `src/` check the two halves
//! separately; this is what proves they agree on the wire.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

/// Cargo runs the tests in one process and in parallel, so each one needs its
/// own socket: two tests sharing a path unlink each other's listener and the
/// hook connects to whichever file happens to be there.
static SLOT: AtomicU32 = AtomicU32::new(0);

fn scratch(tag: &str) -> std::path::PathBuf {
    let n = SLOT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("coucou-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Runs coucou-hook with `args` and `payload` on stdin, while a server on the
/// socket answers with `reply` (or sends nothing when it is None).
fn run(args: &[&str], payload: &str, reply: Option<String>) -> String {
    let dir = scratch("ask-test");
    std::fs::create_dir_all(dir.join("coucou")).unwrap();
    let sock = dir.join("coucou/coucou.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        if let Some(r) = reply {
            stream.write_all(format!("{r}\n").as_bytes()).unwrap();
            stream.flush().unwrap();
            // Hold the connection open until the hook has read the answer.
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        line
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_coucou-hook"))
        .args(args)
        .env("XDG_RUNTIME_DIR", &dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    server.join().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const QUESTION: &str = r#"{"hook_event_name":"PreToolUse","tool_name":"AskUserQuestion","session_id":"s1","tool_input":{"questions":[{"header":"Deploy","question":"Deploy to production?","options":[{"label":"yes","description":"ship it"},{"label":"no","description":"stop"}],"multiSelect":false}]}}"#;

#[test]
fn an_answer_comes_back_as_an_allowed_call() {
    let out = run(
        &["--ask", "PreToolUse"],
        QUESTION,
        Some(r#"{"decision":"answer","answers":{"Deploy to production?":"yes"}}"#.to_string()),
    );
    assert!(out.contains(r#""hookEventName":"PreToolUse""#), "{out}");
    assert!(out.contains(r#""permissionDecision":"allow""#), "{out}");
    assert!(out.contains(r#""answers":{"Deploy to production?":"yes"}"#), "{out}");
    // The original questions are echoed back, or Claude Code has nothing to
    // match the answers against.
    assert!(out.contains("Deploy to production?"), "{out}");
}

#[test]
fn reply_in_terminal_prints_nothing_at_all() {
    // "ask" is what the island sends when the user chooses the terminal: the
    // hook must stay silent so Claude Code asks the question itself.
    let out = run(&["--ask", "PreToolUse"], QUESTION, Some(r#"{"decision":"ask"}"#.to_string()));
    assert_eq!(out, "", "expected silence, got {out}");
}

#[test]
fn a_closed_app_prints_nothing_and_exits_zero() {
    // No server is listening at all — the hook gives up immediately rather than
    // spending its budget, and Claude Code asks in the terminal.
    let dir = scratch("ask-empty");
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_coucou-hook"))
        .args(["--ask", "PreToolUse"])
        .env("XDG_RUNTIME_DIR", &dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(QUESTION.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty());
}

#[test]
fn the_general_pre_tool_use_hook_never_waits_for_an_answer() {
    // Same payload, no --ask: this is the ordinary 10 s entry, and it must not
    // hold the session open even though the question tool is involved.
    let dir = scratch("ask-general");
    std::fs::create_dir_all(dir.join("coucou")).unwrap();
    let sock = dir.join("coucou/coucou.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        line
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_coucou-hook"))
        .args(["PreToolUse"])
        .env("XDG_RUNTIME_DIR", &dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(QUESTION.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let forwarded = server.join().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty());
    assert!(forwarded.contains("AskUserQuestion"), "event was not forwarded: {forwarded}");
}
