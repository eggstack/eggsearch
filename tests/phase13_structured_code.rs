#![cfg(feature = "mock")]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eggsearch::core::code_evidence::SymbolKind;
use eggsearch::core::local::{LocalConfig, LocalSearchRequest};
use eggsearch::meta::local_backend::{
    LocalWorkspaceBackend, StructuredSymbolBackend, SymbolBackend,
};
use eggsearch::meta::local_symbols::{self, SymbolProvenance, TestHintConfidence};
use eggsearch::meta::repo_mapper::build_local_structure;

fn structured_config(root: &std::path::Path) -> LocalConfig {
    LocalConfig {
        enabled: true,
        roots: vec![root.to_path_buf()],
        respect_gitignore: false,
        ..Default::default()
    }
}

fn write_rust_workspace(root: &std::path::Path) {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[workspace]\nmembers = [\"crates/api\"]\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub mod store;\nuse crate::store::Store;\n\npub trait Runner {\n    fn run(&self);\n}\n\npub struct Engine {\n    store: Store,\n}\n\nimpl Runner for Engine {\n    fn run(&self) {}\n}\n\nimpl Engine {\n    pub fn start(&self) {}\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/store.rs"),
        "pub struct Store;\nimpl Store {\n    pub fn save(&self) {}\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(
        root.join("tests/store.rs"),
        "#[test]\nfn test_save() {\n    let s = 1;\n    assert_eq!(s, 1);\n}\n",
    )
    .unwrap();
}

fn write_python_package(root: &std::path::Path) {
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::write(
        root.join("pkg/__init__.py"),
        "import os\n\nclass Store:\n    def save(self):\n        pass\n\ndef load():\n    pass\n",
    )
    .unwrap();
    std::fs::write(
        root.join("pkg/test_store.py"),
        "def test_save():\n    assert True\n",
    )
    .unwrap();
}

fn write_js_package(root: &std::path::Path) {
    std::fs::write(
        root.join("package.json"),
        "{\"name\": \"demo\", \"version\": \"1.0.0\"}",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/api.ts"),
        "import { Store } from './store';\nexport interface Runner {\n    run(): void;\n}\nexport class Api implements Runner {\n    run() {}\n}\nexport function fetchData() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/api.test.ts"),
        "import { Api } from './api';\ndescribe('api', () => {\n    it('works', () => {});\n});",
    )
    .unwrap();
}

fn write_go_module(root: &std::path::Path) {
    std::fs::write(root.join("go.mod"), "module example.com/demo\n\ngo 1.21\n").unwrap();
    std::fs::write(
        root.join("store.go"),
        "package store\n\ntype Store struct{}\n\nfunc New() *Store { return &Store{} }\n\nfunc (s Store) Save() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("store_test.go"),
        "package store\n\nfunc TestSave() {}\n",
    )
    .unwrap();
}

#[test]
fn rust_workspace_structured_facts() {
    let dir = tempfile::tempdir().unwrap();
    write_rust_workspace(dir.path());
    let text = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    let (symbols, provenance, truncated) =
        local_symbols::symbols_in_file("src/lib.rs", Some("rust"), &text, 262_144, 256);
    assert_eq!(provenance, SymbolProvenance::Structured);
    assert!(!truncated);
    assert!(symbols
        .iter()
        .any(|s| s.name == "Engine" && s.kind == SymbolKind::Struct));
    assert!(symbols
        .iter()
        .any(|s| s.name == "Runner" && s.kind == SymbolKind::Trait));
    assert!(symbols
        .iter()
        .any(|s| s.name == "start" && s.kind == SymbolKind::Method));
    let implementors = local_symbols::find_implementors(&symbols, "Engine");
    assert!(!implementors.is_empty());
    let def = local_symbols::find_definition(&symbols, "Engine").unwrap();
    assert_eq!(def.name, "Engine");
    let enclosing = local_symbols::find_enclosing_symbol(&symbols, def.line_start).unwrap();
    assert_eq!(enclosing.name, "Engine");
}

#[test]
fn python_package_structured_facts() {
    let dir = tempfile::tempdir().unwrap();
    write_python_package(dir.path());
    let text = std::fs::read_to_string(dir.path().join("pkg/__init__.py")).unwrap();
    let (symbols, provenance, _) =
        local_symbols::symbols_in_file("pkg/__init__.py", Some("python"), &text, 262_144, 256);
    assert_eq!(provenance, SymbolProvenance::Structured);
    assert!(symbols
        .iter()
        .any(|s| s.name == "Store" && s.kind == SymbolKind::Class));
    assert!(symbols
        .iter()
        .any(|s| s.name == "save" && s.kind == SymbolKind::Method));
    assert!(symbols.iter().any(|s| s.name == "load"));
}

#[test]
fn js_package_structured_facts() {
    let dir = tempfile::tempdir().unwrap();
    write_js_package(dir.path());
    let text = std::fs::read_to_string(dir.path().join("src/api.ts")).unwrap();
    let (symbols, provenance, _) =
        local_symbols::symbols_in_file("src/api.ts", Some("typescript"), &text, 262_144, 256);
    assert_eq!(provenance, SymbolProvenance::Structured);
    assert!(symbols
        .iter()
        .any(|s| s.name == "Api" && s.kind == SymbolKind::Class));
    assert!(symbols
        .iter()
        .any(|s| s.name == "Runner" && s.kind == SymbolKind::Interface));
    assert!(symbols.iter().any(|s| s.name == "fetchData"));
}

#[test]
fn go_module_structured_facts() {
    let dir = tempfile::tempdir().unwrap();
    write_go_module(dir.path());
    let text = std::fs::read_to_string(dir.path().join("store.go")).unwrap();
    let (symbols, provenance, _) =
        local_symbols::symbols_in_file("store.go", Some("go"), &text, 262_144, 256);
    assert_eq!(provenance, SymbolProvenance::Structured);
    assert!(symbols
        .iter()
        .any(|s| s.name == "Store" && s.kind == SymbolKind::Struct));
    assert!(symbols.iter().any(|s| s.name == "New"));
    let method = symbols.iter().find(|s| s.name == "Save").unwrap();
    assert_eq!(method.kind, SymbolKind::Method);
}

#[test]
fn structured_definition_outranks_lexical_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("engine.rs"),
        "pub struct Engine {\n    name: String,\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("notes.md"),
        "This document discusses the engine at length.\n",
    )
    .unwrap();
    let config = structured_config(dir.path());
    let backend = LocalWorkspaceBackend::new(config).unwrap();
    let req = LocalSearchRequest {
        query: "engine".to_string(),
        symbol: Some("Engine".to_string()),
        timeout_ms: Some(10_000),
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(backend.search(&req));
    assert!(!result.matches.is_empty());
    let top = &result.matches[0];
    assert_eq!(top.matched_symbol.as_deref(), Some("Engine"));
    assert_eq!(top.symbol_provenance.as_deref(), Some("structured"));
    assert_eq!(top.is_exact_definition, Some(true));
}

#[test]
fn regex_fallback_remains_functional() {
    let backend = StructuredSymbolBackend::default();
    let text = "this file has no parseable definitions for xyz language";
    let hit = backend.find_symbols(text, "missing_symbol_xyz");
    assert!(hit.is_none());
    let caps = backend.capabilities();
    assert!(caps.supports_definitions);
    assert!(caps.regex_fallback_available);
    assert!(caps.structured_languages.contains(&"rust".to_string()));
}

#[test]
fn parser_budget_breach_degrades_cleanly() {
    let text = "fn a() {}".repeat(500);
    let (symbols, provenance, truncated) =
        local_symbols::symbols_in_file("a.rs", Some("rust"), &text, 16, 256);
    assert!(symbols.is_empty());
    assert_eq!(provenance, SymbolProvenance::RegexFallback);
    assert!(truncated);
    let refs = local_symbols::find_references(&text, "a", 5);
    assert!(refs.len() <= 5);
}

#[test]
fn repo_map_structure_is_bounded_and_additive() {
    let dir = tempfile::tempdir().unwrap();
    write_rust_workspace(dir.path());
    write_python_package(dir.path());
    write_js_package(dir.path());
    write_go_module(dir.path());
    let config = structured_config(dir.path());
    let structure = build_local_structure(dir.path(), &config);
    assert!(!structure.language_distribution.is_empty());
    assert!(!structure.entrypoints.is_empty());
    assert!(!structure.top_symbols.is_empty());
    assert!(!structure.packages.is_empty());
    assert!(structure.top_symbols.len() <= config.repo_map_structure_cap);
    assert!(structure.test_relationships.len() <= 100);
    for rel in &structure.test_relationships {
        assert!(!rel.source_path.is_empty());
        assert!(!rel.test_path.is_empty());
        assert!(["syntax", "path", "name_reference", "package"].contains(&rel.confidence.as_str()));
        assert!(!rel.reasons.is_empty());
    }
}

#[test]
fn related_test_hints_label_heuristics() {
    let mut texts = HashMap::new();
    texts.insert(
        "tests/store.rs".to_string(),
        "#[test]\nfn test_save() {}".to_string(),
    );
    let hints = local_symbols::related_test_hints(
        "src/store.rs",
        &["save".to_string(), "Store".to_string()],
        &["tests/store.rs".to_string()],
        &texts,
    );
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].confidence, TestHintConfidence::Syntax);
    assert!(hints[0].reasons.contains(&"syntax_test_item".to_string()));
}

#[test]
fn structured_search_telemetry_distinguishes_provenance() {
    let dir = tempfile::tempdir().unwrap();
    write_rust_workspace(dir.path());
    let config = structured_config(dir.path());
    let backend = LocalWorkspaceBackend::new(config).unwrap();
    let req = LocalSearchRequest {
        query: "Engine".to_string(),
        symbol: Some("Engine".to_string()),
        timeout_ms: Some(10_000),
        ..Default::default()
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(backend.search(&req));
    let telemetry = result.telemetry.as_ref().unwrap();
    assert!(telemetry.structured_files_parsed > 0);
    assert!(telemetry.structured_symbols_found > 0);
}

#[test]
fn no_workspace_code_execution_in_parsing() {
    let text = "fn evil() { std::process::Command::new(\"rm\").spawn(); }\n";
    let start = Instant::now();
    let (symbols, provenance, _) =
        local_symbols::symbols_in_file("evil.rs", Some("rust"), text, 262_144, 256);
    assert_eq!(provenance, SymbolProvenance::Structured);
    assert!(symbols.iter().any(|s| s.name == "evil"));
    assert!(start.elapsed() < Duration::from_secs(5));
}
