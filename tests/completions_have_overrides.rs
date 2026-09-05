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

#[test]
fn zsh_profile_positions_use_dynamic_completer() {
    let body = fs::read_to_string("completions/_envio")
        .expect("zsh completion script missing — run `cargo build` first");
    assert!(
        body.contains(":_envio_profiles'"),
        "zsh profile_name positionals lost their `_envio_profiles` action — \
         check the injector in build.rs against the generated arg specs"
    );
    assert!(
        body.contains(":PROFILE_NAME:_envio_profiles "),
        "zsh `list -n` lost its `_envio_profiles` action"
    );
}

#[test]
fn completions_describe_every_option() {
    let zsh = fs::read_to_string("completions/_envio")
        .expect("zsh completion script missing — run `cargo build` first");
    assert!(
        !zsh.contains("[]"),
        "zsh completion has options with an empty description — \
         add `help = \"...\"` to the matching arg in clap_app.rs"
    );

    let fish = fs::read_to_string("completions/envio.fish")
        .expect("fish completion script missing — run `cargo build` first");
    let undescribed: Vec<&str> = fish
        .lines()
        .filter(|l| l.starts_with("complete "))
        .filter(|l| l.contains(" -s ") || l.contains(" -l "))
        .filter(|l| !l.contains(" -d "))
        .collect();
    assert!(
        undescribed.is_empty(),
        "fish completion has options with no description: {:#?}",
        undescribed
    );
}
