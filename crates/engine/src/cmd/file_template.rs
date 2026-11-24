//! Design templates: save the open design into the templates folder, list what is there, copy a
//! design in or out, and delete one.
//!
//! A template is an ordinary design file — `recovery::data_dir()/templates/<name>.sc`, beside
//! `recovery/`, `plugins/` and `logs/`. Nothing here invents a format, and every file call goes
//! through `solidcraft_io::vfs`, which is the same call on the desktop and in the browser.

use super::CommandSpec;
use crate::schema::file_template::{
    TemplateDeleteParams, TemplateDeleted, TemplateExportParams, TemplateImportParams, TemplateInfo, TemplateListParams, TemplateListed,
    TemplateSaveParams, TemplateWritten,
};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session, params::bad};
use serde_json::Value;
use solidcraft_doc::Document;

const SAVE_CONTRACT: TypedContract<TemplateSaveParams> = TypedContract::new();
const LIST_CONTRACT: TypedContract<TemplateListParams> = TypedContract::new();
const IMPORT_CONTRACT: TypedContract<TemplateImportParams> = TypedContract::new();
const EXPORT_CONTRACT: TypedContract<TemplateExportParams> = TypedContract::new();
const DELETE_CONTRACT: TypedContract<TemplateDeleteParams> = TypedContract::new();

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("file.template.save", "Save As Template", crate::checked!("file.template.save", save))
        .icon("save")
        .contract(&SAVE_CONTRACT)
        .output(output_schema_for::<TemplateWritten>)
        .params("name?: the template's name (default: the design's); description?: the short line the gallery shows (it is kept on the design); overwrite? false: replace a template of that name. Writes a copy: the design is not renamed, and is not pointed at the template"),
    CommandSpec::new("file.template.list", "List Templates", crate::checked!("file.template.list", list))
        .icon("open")
        .noundo()
        .doc_only()
        .contract(&LIST_CONTRACT)
        .output(output_schema_for::<TemplateListed>)
        .params("no parameters; every saved template with its name, description, units and path. Read without opening a design"),
    CommandSpec::new("file.template.import", "Import Template", crate::checked!("file.template.import", import))
        .icon("import")
        .noundo()
        .doc_only()
        .contract(&IMPORT_CONTRACT)
        .output(output_schema_for::<TemplateWritten>)
        .params("path: the design file to copy into the templates folder; name?: what to call it (default: the file's name); overwrite? false. Refuses a file that is not a design"),
    CommandSpec::new("file.template.export", "Export Template", crate::checked!("file.template.export", export))
        .icon("export")
        .noundo()
        .doc_only()
        .contract(&EXPORT_CONTRACT)
        .output(output_schema_for::<TemplateWritten>)
        .params("name: the template, as file.template.list gives it; path: where to write the copy; overwrite? false. A copy is byte-identical, so it keeps the template's embedded picture"),
    CommandSpec::new("file.template.delete", "Delete Template", crate::checked!("file.template.delete", delete))
        .icon("delete")
        .noundo()
        .doc_only()
        .contract(&DELETE_CONTRACT)
        .output(output_schema_for::<TemplateDeleted>)
        .params("name: the template to remove. Designs already made from it are untouched"),
];

/// The longest a template name may be, as a file name.
const MAX_NAME: usize = 128;
/// The longest a template's description may be.
const MAX_DESCRIPTION: usize = 200;
/// The most templates one listing returns.
const MAX_TEMPLATES: usize = 2000;

/// The templates folder this process uses instead of the application data folder. Tests point it
/// at a temporary folder (and hold [`TEMPLATES_TEST_LOCK`] while they do: one process, one
/// folder); the app leaves it unset.
#[cfg(test)]
pub(crate) static TEMPLATES_DIR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// Serialises the tests that point [`TEMPLATES_DIR`] somewhere, since they share one process.
#[cfg(test)]
pub(crate) static TEMPLATES_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The templates folder.
fn folder(cmd: &str) -> Result<String> {
    #[cfg(test)]
    if let Some(dir) = TEMPLATES_DIR.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(dir);
    }
    crate::recovery::data_dir()
        .map(|dir| dir.join("templates"))
        .map(|dir| dir.to_string_lossy().replace('\\', "/"))
        .filter(|dir| !dir.is_empty())
        .ok_or_else(|| bad(cmd, "this system has no application data folder, so templates have nowhere to live"))
}

/// A template's name is a file name: trimmed, an extension dropped, and nothing that would make
/// it a path.
fn name_of(cmd: &str, wanted: &str) -> Result<String> {
    let trimmed = wanted.trim();
    let stem = trimmed.strip_suffix(".sc").or_else(|| trimmed.strip_suffix(".solidcraft")).unwrap_or(trimmed).trim();
    if stem.is_empty() {
        return Err(bad(cmd, "a template needs a name"));
    }
    if stem.chars().count() > MAX_NAME {
        return Err(bad(cmd, "a template's name is at most 128 characters"));
    }
    if stem == "." || stem == ".." || stem.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err(bad(cmd, "a template's name is a file name: no path separators"));
    }
    Ok(stem.to_string())
}

/// The file a template is saved at.
fn file_of(cmd: &str, name: &str) -> Result<String> {
    let path = format!("{}/{name}.sc", folder(cmd)?);
    solidcraft_io::vfs::check(&path).map_err(|e| bad(cmd, e.to_string()))?;
    Ok(path)
}

/// A description, cut to what the gallery shows.
fn description_of(cmd: &str, text: &str) -> Result<String> {
    let trimmed = text.trim();
    if trimmed.chars().count() > MAX_DESCRIPTION {
        return Err(bad(cmd, "a template's description is at most 200 characters"));
    }
    Ok(trimmed.to_string())
}

fn reply(cmd: &str, value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| bad(cmd, e.to_string()))
}

/// Write `bytes` to `path`, making the templates folder first. Without `overwrite` an existing
/// file is refused rather than replaced: the app asks the user, then calls again with true.
fn write(cmd: &str, path: &str, bytes: &[u8], overwrite: bool) -> Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        let parent = parent.to_string_lossy().replace('\\', "/");
        solidcraft_io::vfs::create_dir_all(&parent).map_err(|e| bad(cmd, format!("could not make the templates folder: {e}")))?;
    }
    let file = std::path::Path::new(path);
    let result = if overwrite {
        solidcraft_io::safe_write::write_atomic(file, bytes, false)
    } else {
        solidcraft_io::safe_write::create_atomic(file, bytes)
    };
    result.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            bad(cmd, format!("{path}: a file of that name is already there; pass `overwrite: true` to replace it"))
        } else {
            bad(cmd, format!("could not write {path}: {e}"))
        }
    })
}

/// The design a template holds, for `file.new {template}`: read the way opening a design reads,
/// so the new design is the template's content and nothing about the template file changes.
pub(crate) fn template_document(name: &str) -> Result<Document> {
    let cmd = "file.new";
    let name = name_of(cmd, name)?;
    let path = file_of(cmd, &name)?;
    if !solidcraft_io::vfs::is_file(&path) {
        return Err(bad(cmd, format!("{name}: no such template")));
    }
    solidcraft_io::read_design_file(&path, None::<fn(solidcraft_io::DesignPreview)>)
        .map(|(document, _)| document)
        .map_err(|e| bad(cmd, format!("{name}: {e}")))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "file.template.save";
    let request: TemplateSaveParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let name = name_of(cmd, request.name.as_deref().unwrap_or(&s.doc.name))?;
    let path = file_of(cmd, &name)?;
    let overwrite = request.overwrite.unwrap_or(false);
    // The description lives on the design — that is what makes it reachable when the design is
    // opened on its own — so the copy and the design agree.
    if let Some(text) = request.description.as_deref() {
        let description = description_of(cmd, text)?;
        if s.doc.description != description {
            s.doc_mut().description = description;
        }
    }
    let bytes = s
        .container_bytes(Some(&path), solidcraft_io::DesignCompression::Fast, false, true, None, true)
        .map_err(|e| bad(cmd, format!("could not save the template: {e}")))?;
    write(cmd, &path, &bytes, overwrite)?;
    reply(cmd, TemplateWritten { name, path, bytes: bytes.len(), param_warnings: Vec::new() })
}

fn list(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "file.template.list";
    let _request: TemplateListParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let folder = folder(cmd)?;
    // No folder yet is an empty gallery, not a failure; a folder that cannot be read is a
    // failure, not an empty one.
    let files = if solidcraft_io::vfs::exists(&folder) {
        solidcraft_io::vfs::files_in(&folder, MAX_TEMPLATES).map_err(|e| bad(cmd, format!("{folder}: {e}")))?
    } else {
        Vec::new()
    };
    let mut templates = Vec::new();
    for path in files {
        let file = std::path::Path::new(&path);
        if !file.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("sc") || e.eq_ignore_ascii_case("solidcraft"))
        {
            continue;
        }
        // The file's own part alone: naming a template never opens one.
        let metadata = solidcraft_io::read_design_metadata_file(&path);
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let described = metadata.as_ref().map(|m| m.description.trim().to_string()).filter(|d| !d.is_empty());
        let modified = solidcraft_io::vfs::stamp(&path)
            .and_then(|(time, _)| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_secs());
        templates.push(TemplateInfo {
            // The template's name is its file's: that is what save wrote, and what export and
            // delete take. The design's own name belongs to the copy, not to the template.
            name: stem,
            path,
            description: described,
            units: metadata.map(|m| m.units).unwrap_or_default(),
            modified,
        });
    }
    templates.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    reply(cmd, TemplateListed { templates, folder, param_warnings: Vec::new() })
}

fn import(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "file.template.import";
    let request: TemplateImportParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    solidcraft_io::vfs::check(&request.path).map_err(|e| bad(cmd, e.to_string()))?;
    let fallback = std::path::Path::new(&request.path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = name_of(cmd, request.name.as_deref().unwrap_or(&fallback))?;
    let path = file_of(cmd, &name)?;
    // What makes a file a template: it is a design. A `.sc` that is not one is refused here
    // rather than landing in the folder to fail later.
    if solidcraft_io::read_design_metadata_file(&request.path).is_none() {
        return Err(bad(cmd, format!("{}: not a SolidCraft design", request.path)));
    }
    let bytes = solidcraft_io::vfs::read(&request.path, solidcraft_io::limits::max_design_bytes())
        .map_err(|e| bad(cmd, format!("could not read {}: {e}", request.path)))?;
    write(cmd, &path, &bytes, request.overwrite.unwrap_or(false))?;
    reply(cmd, TemplateWritten { name, path, bytes: bytes.len(), param_warnings: Vec::new() })
}

fn export(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "file.template.export";
    let request: TemplateExportParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let name = name_of(cmd, &request.name)?;
    let source = file_of(cmd, &name)?;
    solidcraft_io::vfs::check(&request.path).map_err(|e| bad(cmd, e.to_string()))?;
    let bytes = solidcraft_io::vfs::read(&source, solidcraft_io::limits::max_design_bytes())
        .map_err(|e| bad(cmd, format!("{source}: {e}")))?;
    write(cmd, &request.path, &bytes, request.overwrite.unwrap_or(false))?;
    reply(cmd, TemplateWritten { name, path: request.path, bytes: bytes.len(), param_warnings: Vec::new() })
}

fn delete(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "file.template.delete";
    let request: TemplateDeleteParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let name = name_of(cmd, &request.name)?;
    let path = file_of(cmd, &name)?;
    if !solidcraft_io::vfs::is_file(&path) {
        return Err(bad(cmd, format!("{name}: no such template")));
    }
    solidcraft_io::vfs::remove(&path).map_err(|e| bad(cmd, format!("{path}: {e}")))?;
    reply(cmd, TemplateDeleted { name, path, param_warnings: Vec::new() })
}

#[cfg(test)]
#[path = "file_template_tests.rs"]
mod tests;
