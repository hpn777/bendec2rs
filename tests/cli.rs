use std::{
    io::Write,
    process::{Command, Stdio},
};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bendec2rs"))
}

#[test]
fn stdin_and_batch_agree() {
    let request = r#"{"types":[{"name":"Id","alias":"u64"}],"options":{}}"#;
    let mut child = command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.as_bytes())
        .unwrap();
    let single = child.wait_with_output().unwrap();
    assert!(single.status.success());
    let mut child = command()
        .arg("--batch")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(child.stdin.take().unwrap(), "{request}\n{request}").unwrap();
    let batch = child.wait_with_output().unwrap();
    assert!(batch.status.success());
    let lines: Vec<_> = batch
        .stdout
        .split(|&b| b == b'\n')
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(lines.len(), 2);
    for line in lines {
        assert_eq!(
            serde_json::from_slice::<String>(line).unwrap().as_bytes(),
            single.stdout
        );
    }
}

#[test]
fn refuses_existing_output_without_touching_it() {
    let dir = std::env::temp_dir().join(format!("bendec-cli-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let output = dir.join("keep.rs");
    std::fs::write(&output, "user work").unwrap();
    let mut child = command()
        .arg("--output")
        .arg(&output)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"[]").unwrap();
    assert!(!child.wait_with_output().unwrap().status.success());
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "user work");
    std::fs::remove_file(output).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn rejects_bad_input_and_ambiguous_arguments() {
    for args in [
        vec!["--bogus"],
        vec!["--output"],
        vec!["--batch", "input.json"],
    ] {
        assert!(!command().args(args).output().unwrap().status.success());
    }
    let mut child = command()
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    assert!(!child.wait_with_output().unwrap().status.success());
}
