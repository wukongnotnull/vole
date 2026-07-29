use std::process::Command;

#[test]
fn completions_zsh_prints_script() {
    let output = Command::new(env!("CARGO_BIN_EXE_vole"))
        .args(["completions", "zsh"])
        .output()
        .expect("run vole completions");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("vole") || stdout.contains("_vole"),
        "unexpected completion script: {}",
        &stdout[..stdout.len().min(200)]
    );
}
