use std::fs;

const SENTINEL: &str = "# envio: dynamic profile completion END";

#[test]
fn bash_completion_has_override() {
    let body = fs::read_to_string("completions/envio.bash")
        .expect("bash completion script missing — run `cargo build` first");
    assert!(
        body.contains(SENTINEL),
        "bash completion missing sentinel `{}`",
        SENTINEL
    );
}

#[test]
fn zsh_completion_has_override() {
    let body = fs::read_to_string("completions/_envio")
        .expect("zsh completion script missing — run `cargo build` first");
    assert!(
        body.contains(SENTINEL),
        "zsh completion missing sentinel `{}`",
        SENTINEL
    );
}

#[test]
fn fish_completion_has_override() {
    let body = fs::read_to_string("completions/envio.fish")
        .expect("fish completion script missing — run `cargo build` first");
    assert!(
        body.contains(SENTINEL),
        "fish completion missing sentinel `{}`",
        SENTINEL
    );
}

#[test]
fn powershell_completion_has_override() {
    let body = fs::read_to_string("completions/_envio.ps1")
        .expect("powershell completion script missing — run `cargo build` first");
    assert!(
        body.contains(SENTINEL),
        "powershell completion missing sentinel `{}`",
        SENTINEL
    );
}
