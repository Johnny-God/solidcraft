//! Tests for `file.template.*`: the store round trip, importing and exporting a template, and
//! the description the design carries.
//!
//! The tests point the templates folder at a temporary folder. They share one process, so the
//! lock in [`Templates`] is what keeps them from seeing each other's folder.

use super::*;
use crate::cmd::file_template::{TEMPLATES_DIR, TEMPLATES_TEST_LOCK};
use serde_json::json;

/// A temporary templates folder, held for one test's life.
struct Templates {
    _lock: std::sync::MutexGuard<'static, ()>,
    dir: std::path::PathBuf,
}

impl Templates {
    fn new(name: &str) -> Self {
        let lock = TEMPLATES_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = solidcraft_io::vfs::temp_dir().join(format!("solidcraft-templates-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        *TEMPLATES_DIR.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_string_lossy().replace('\\', "/"));
        Self { _lock: lock, dir }
    }
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Templates {
    fn drop(&mut self) {
        *TEMPLATES_DIR.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A scratch folder beside the templates one, for the files these tests import and export.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = solidcraft_io::vfs::temp_dir().join(format!("solidcraft-template-scratch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session() -> Session {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length":10,"width":10,"height":10})).unwrap();
    s.mark_saved();
    s
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap()
}

fn metadata(path: &std::path::Path) -> solidcraft_io::DesignMetadata {
    solidcraft_io::read_design_metadata_file(&path.to_string_lossy()).unwrap()
}

#[test]
fn templates_round_trip() {
    let templates = Templates::new("round-trip");
    let mut s = session();
    s.doc_mut().units = "in".into();

    let saved = run(&mut s, "file.template.save", json!({"name":"Bracket","description":"A right-angle bracket"}));
    assert_eq!(saved["name"], "Bracket");
    let path = templates.path("Bracket.sc");
    assert!(path.is_file(), "{path:?}");
    // The copy keeps the design's own units, so the gallery can say what a template is in.
    assert_eq!(metadata(&path).units, "in");
    assert_eq!(metadata(&path).description(), Some("A right-angle bracket"));

    // The same name again is refused rather than replaced, and the file is left as it was.
    let before = std::fs::read(&path).unwrap();
    let refused = s.execute("file.template.save", &json!({"name":"Bracket"})).unwrap_err().to_string();
    assert!(refused.contains("overwrite: true"), "{refused}");
    assert_eq!(std::fs::read(&path).unwrap(), before);

    // With `overwrite` it replaces the template.
    run(&mut s, "file.template.save", json!({"name":"Bracket","description":"Now a flange","overwrite":true}));
    assert_eq!(metadata(&path).description(), Some("Now a flange"));

    let listed = run(&mut s, "file.template.list", json!({}));
    let found = listed["templates"].as_array().unwrap();
    assert_eq!(found.len(), 1, "{listed}");
    assert_eq!(found[0]["name"], "Bracket");
    assert_eq!(found[0]["description"], "Now a flange");
    assert_eq!(found[0]["units"], "in");
    assert!(found[0]["path"].as_str().unwrap().ends_with("Bracket.sc"));
    assert!(found[0]["modified"].as_u64().is_some(), "{listed}");

    // Deleting a name that is not there is refused, and deleting the template removes the file.
    let missing = s.execute("file.template.delete", &json!({"name":"Nothing"})).unwrap_err().to_string();
    assert!(missing.contains("no such template"), "{missing}");
    run(&mut s, "file.template.delete", json!({"name":"Bracket"}));
    assert!(!path.is_file());
    assert_eq!(run(&mut s, "file.template.list", json!({}))["templates"].as_array().unwrap().len(), 0);
}

#[test]
fn template_import_export_byte_identical() {
    let templates = Templates::new("import-export");
    let dir = scratch("import-export");
    let mut s = session();

    // A design saved by the design's own writer, outside the templates folder.
    let source = dir.join("outside.sc");
    run(&mut s, "file.save", json!({"path": source, "overwrite": true}));
    let bytes = std::fs::read(&source).unwrap();

    // A file that is not a design is refused rather than landing in the folder.
    let stranger = dir.join("notes.sc");
    std::fs::write(&stranger, b"not a design at all").unwrap();
    let refused = s.execute("file.template.import", &json!({"path": stranger})).unwrap_err().to_string();
    assert!(refused.contains("not a SolidCraft design"), "{refused}");

    // Importing a design copies it in byte for byte, so it keeps its embedded picture.
    run(&mut s, "file.template.import", json!({"path": source, "name":"Imported"}));
    let landed = templates.path("Imported.sc");
    assert!(landed.is_file());
    assert_eq!(std::fs::read(&landed).unwrap(), bytes);

    // Exporting copies it out again, byte for byte.
    let out = dir.join("exported.sc");
    run(&mut s, "file.template.export", json!({"name":"Imported","path": out}));
    assert_eq!(std::fs::read(&out).unwrap(), bytes);

    // Exporting respects `overwrite`, like importing does.
    let refused = s.execute("file.template.export", &json!({"name":"Imported","path": out})).unwrap_err().to_string();
    assert!(refused.contains("overwrite: true"), "{refused}");
    run(&mut s, "file.template.export", json!({"name":"Imported","path": out, "overwrite": true}));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_design_from_template() {
    let templates = Templates::new("new-design");
    let mut s = session();
    run(&mut s, "file.template.save", json!({"name":"Box","description":"A 10 mm cube"}));
    let path = templates.path("Box.sc");
    let template_bytes = std::fs::read(&path).unwrap();
    let content: Vec<String> = s.doc.features.iter().map(|feature| feature.name.clone()).collect();
    assert!(!content.is_empty(), "the design the template was saved from has a feature");

    // A new design from the template holds its content, unsaved, under the default name.
    let mut fresh = session();
    run(&mut fresh, "file.new", json!({"template":"Box"}));
    assert_eq!(fresh.doc.name, "Untitled");
    assert!(fresh.path.is_none(), "a design from a template is unsaved: {:?}", fresh.path);
    assert_eq!(fresh.doc.features.iter().map(|feature| feature.name.clone()).collect::<Vec<_>>(), content);
    assert_eq!(fresh.doc.metadata.description(), Some("A 10 mm cube"));

    // Reading the template changed nothing about it.
    assert_eq!(std::fs::read(&path).unwrap(), template_bytes);

    // A template that is not there is refused, and the design is left as it was.
    let before = fresh.doc.clone();
    let refused = fresh.execute("file.new", &json!({"template":"Nothing"})).unwrap_err().to_string();
    assert!(refused.contains("no such template"), "{refused}");
    assert!(std::sync::Arc::ptr_eq(&before, &fresh.doc));
}

#[test]
fn description_round_trip() {
    let templates = Templates::new("description");
    let dir = scratch("description");
    let mut s = session();

    s.doc_mut()
        .metadata
        .set(solidcraft_doc::metadata::Owner::Design, solidcraft_doc::metadata::DESCRIPTION, json!("A plate with four holes"))
        .unwrap();
    let described = templates.path("Plate.sc");
    run(&mut s, "file.save", json!({"path": described, "overwrite": true}));
    assert_eq!(metadata(&described).description(), Some("A plate with four holes"));

    // A design saved without a description reads back with none, and nothing is written for it.
    let plain = dir.join("Plain.sc");
    let mut bare = session();
    run(&mut bare, "file.save", json!({"path": plain, "overwrite": true}));
    assert_eq!(metadata(&plain).description(), None);

    let _ = std::fs::remove_dir_all(&dir);
}
