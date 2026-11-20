use std::process::Command;

use eggsearch::core::identity::canonicalize_url;
use eggsearch::core::knowledge_delta::KnowledgeIndexConfig;
use eggsearch::meta::knowledge_delta::{EvidenceIndex, IndexError, IndexRecord};
use rusqlite::Connection;

fn config() -> KnowledgeIndexConfig {
    KnowledgeIndexConfig {
        enabled: true,
        path: None,
        max_bytes: 1024 * 1024,
        max_documents: 10,
        max_excerpt_chars: 80,
        retention_days: 365,
    }
}

fn record(id: &str, partition: &str) -> IndexRecord {
    IndexRecord {
        partition: partition.into(),
        canonical_url: format!("https://example.com/{id}"),
        source_id: format!("src_{id}"),
        title: "widget-runtime notes".into(),
        excerpt: "dated public evidence".into(),
        trust: "external_untrusted".into(),
        basis: "publication".into(),
        precision: "month".into(),
        normalized_date: Some("2024-07".into()),
        kdelta_id: format!("kdelta_{id}"),
        conflicted: false,
        temporal_relationship: "after_cutoff".into(),
        topic: "widget-runtime".into(),
        primary_subject: Some("widget-runtime".into()),
    }
}

#[test]
fn precision_survives_restart_and_other_partitions_are_invisible() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    index.retain(&[record("a", "anonymous")]).unwrap();
    drop(index);
    let again = EvidenceIndex::open(&path, &config()).unwrap();
    let hits = again.lookup("anonymous", "widget-runtime", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].record.precision, "month");
    assert_eq!(hits[0].record.normalized_date.as_deref(), Some("2024-07"));
    assert!(again
        .lookup("prof_0123456789abcdef", "widget-runtime", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn private_or_invalid_batch_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    let mut private = record("b", "anonymous");
    private.trust = "profile_scoped".into();
    let err = index
        .retain(&[record("a", "anonymous"), private])
        .expect_err("profile-scoped body");
    assert!(err.message().contains("refusing"));
    assert!(index
        .lookup("anonymous", "widget-runtime", 10)
        .unwrap()
        .is_empty());

    let mut invalid = record("c", "alice");
    invalid.trust = "external_untrusted".into();
    assert!(matches!(
        index.retain(&[record("a", "anonymous"), invalid]),
        Err(IndexError::InvalidPartition)
    ));
    assert!(index
        .lookup("anonymous", "widget-runtime", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn fts_search_is_bounded_and_urls_use_shared_canonicalization() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    let mut item = record("fts", "anonymous");
    item.canonical_url = "https://WWW.Example.COM:443/articles/new#fragment".into();
    item.topic = "widget-runtime".into();
    item.title = "Widget runtime notes".into();
    item.excerpt = "bounded public excerpt".into();
    index.retain(&[item.clone()]).unwrap();
    let hits = index.lookup("anonymous", "runtime widget", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0].record.canonical_url,
        canonicalize_url(&item.canonical_url)
    );
    assert_eq!(hits[0].record.trust, "external_untrusted");
    assert!(index
        .lookup("anonymous", "nonexistenttoken", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn refused_body_classes_never_commit_any_part_of_the_batch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    for trust in ["profile_scoped", "private", "no_store", "pass_cookie"] {
        let mut refused = record(trust, "anonymous");
        refused.trust = trust.into();
        let error = index
            .retain(&[record("good", "anonymous"), refused])
            .expect_err("non-public evidence must be refused");
        assert!(error.message().contains("refusing"));
        assert!(index
            .lookup("anonymous", "widget-runtime", 10)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn max_bytes_returns_full_without_partial_batch_commit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let mut cfg = config();
    cfg.max_bytes = 96 * 1024;
    cfg.max_documents = 1000;
    cfg.max_excerpt_chars = 8_000;
    let index = EvidenceIndex::open(&path, &cfg).expect("schema fits the configured cap");
    let records: Vec<_> = (0..80)
        .map(|number| {
            let mut item = record(&format!("full-{number:03}"), "anonymous");
            item.title = format!("widget-runtime full disk case {number}");
            item.excerpt = format!("term-{number} {}", "bounded evidence ".repeat(450));
            item
        })
        .collect();
    assert!(matches!(index.retain(&records), Err(IndexError::Full(_))));
    assert!(index
        .lookup("anonymous", "widget-runtime", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn read_only_index_reports_a_bounded_write_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    index.retain(&[record("existing", "anonymous")]).unwrap();
    drop(index);
    let original_permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    std::fs::set_permissions(&path, read_only_permissions).unwrap();
    let opened = EvidenceIndex::open(&path, &config());
    let result = match opened {
        Ok(index) => index.retain(&[record("new", "anonymous")]),
        Err(error) => Err(error),
    };
    std::fs::set_permissions(&path, original_permissions).unwrap();
    assert!(matches!(result, Err(IndexError::ReadOnly(_))));
}

#[test]
fn interrupted_sqlite_transaction_rolls_back_on_connection_drop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    drop(index);
    let mut conn = Connection::open(&path).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute(
        "INSERT INTO documents (partition, canonical_url, source_id, title, excerpt, trust, basis, precision, normalized_date, indexed_at) VALUES ('anonymous', 'https://example.com/partial', 'src_partial', 'partial', 'partial', 'external_untrusted', 'publication', 'day', '2024-07-02', 1)",
        [],
    )
    .unwrap();
    drop(tx);
    drop(conn);
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    let count: i64 = Connection::open(&path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
    assert!(index
        .lookup("anonymous", "widget-runtime", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn two_processes_serialize_writes_and_read_the_same_database() {
    const CHILD_PATH: &str = "EGGSEARCH_KD_INDEX_CHILD_PATH";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        let path = std::path::PathBuf::from(path);
        let index = EvidenceIndex::open(&path, &config()).unwrap();
        index.retain(&[record("child", "anonymous")]).unwrap();
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = EvidenceIndex::open(&path, &config()).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("two_processes_serialize_writes_and_read_the_same_database")
        .arg("--nocapture")
        .env(CHILD_PATH, &path)
        .spawn()
        .unwrap();
    index.retain(&[record("parent", "anonymous")]).unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    let hits = index.lookup("anonymous", "widget-runtime", 10).unwrap();
    assert_eq!(hits.len(), 2);
}

#[test]
fn corrupt_file_is_refused_and_a_second_connection_can_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let writer = EvidenceIndex::open(&path, &config()).unwrap();
    writer.retain(&[record("a", "anonymous")]).unwrap();
    let reader = EvidenceIndex::open(&path, &config()).unwrap();
    assert_eq!(
        reader
            .lookup("anonymous", "widget-runtime", 10)
            .unwrap()
            .len(),
        1
    );
    drop(writer);
    drop(reader);

    let junk = dir.path().join("junk.sqlite");
    std::fs::write(&junk, b"not a database").unwrap();
    assert!(EvidenceIndex::open(&junk, &config()).is_err());
}
