use super::*;

#[test]
fn words_of_project() {
    let base = std::env::temp_dir().join(format!("tvs_words_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("src/net")).unwrap();
    std::fs::create_dir_all(base.join("tools/gen/src")).unwrap();
    std::fs::write(
            base.join("Cargo.toml"),
            "[package]\nname = \"app\"\n\n[dependencies]\nserde_json = \"1\"\nmy-lib = { path = \"../x\" }\n\n[dev-dependencies]\npretty-assertions = \"1\"\n\n[workspace]\nmembers = [\"tools/*\"]\n",
        )
        .unwrap();
    std::fs::write(base.join("src/main.rs"), "mod net;\nfn main() { let config_path = load_config(); }\n// x1 ab let\n").unwrap();
    std::fs::write(base.join("src/net/mod.rs"), "pub struct Connection;\n").unwrap();
    std::fs::write(base.join("tools/gen/Cargo.toml"), "[package]\nname = \"gen\"\n[dependencies]\nclap = \"4\"\n").unwrap();
    std::fs::write(base.join("tools/gen/src/codegen.rs"), "pub fn emit_table() {}\n").unwrap();
    let w = project_words(&base);
    for (k, kind) in [
        ("serde_json", KIND_MODULE),
        ("my_lib", KIND_MODULE),
        ("pretty_assertions", KIND_MODULE),
        ("clap", KIND_MODULE),
        ("std", KIND_MODULE),
        ("net", KIND_MODULE),
        ("codegen", KIND_MODULE),
        ("config_path", KIND_TEXT),
        ("load_config", KIND_TEXT),
        ("Connection", KIND_TEXT),
        ("emit_table", KIND_TEXT),
    ] {
        assert_eq!(w.get(k), Some(&kind), "{}", k);
    }
    for no in ["let", "fn", "x1", "ab", "mod"] {
        assert!(!w.contains_key(no), "{}", no);
    }
    let it = items(&w);
    assert_eq!(it[0].kind, KIND_MODULE, "крейты и модули — первыми");
    let _ = std::fs::remove_dir_all(&base);
}
