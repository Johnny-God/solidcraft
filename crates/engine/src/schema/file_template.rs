//! Contracts and result types for the `file.template.*` commands: saving the open design as a
//! template, listing the templates, copying one in or out, and deleting one.
//!
//! A template is an ordinary design file in the templates folder (`recovery::data_dir()`), so
//! nothing here describes a new file format. Naming a template is the command's parameter; the
//! list replies with the file's own name, description and units, read without opening it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Save the open design into the templates folder as a copy: later edits to a template do not
/// change a design made from it, and the open design is not changed, renamed or pointed at the
/// template. Not an undo step.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TemplateSaveParams {
    /// The template's name. Default: the design's own name. A name is a file name: no path
    /// separators, and `.sc` is added for you.
    #[serde(default)]
    pub name: Option<String>,
    /// A short line shown under the name in the gallery. Default: the description the design
    /// already carries, if any. Cut to 200 characters.
    #[serde(default)]
    pub description: Option<String>,
    /// true replaces an existing template of that name. Default: false, and the command refuses
    /// instead — the app asks the user, then calls again with true.
    #[serde(default)]
    pub overwrite: Option<bool>,
}

/// One template, as the gallery lists it.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TemplateInfo {
    /// The template's name, as `file.template.save` took it.
    pub name: String,
    /// The template file's full path.
    pub path: String,
    /// The description the design carries, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The design's units (`mm`, `in`, ...). Empty when the file does not say.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub units: String,
    /// When the template file was last written, in seconds since the Unix epoch (UTC).
    /// Absent when the system does not report it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

/// What `file.template.save`, `file.template.import` and `file.template.export` return.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TemplateWritten {
    /// The template's name: the `name` given, else the design's own.
    pub name: String,
    /// The file written: the template's path for save and import, the chosen path for export.
    pub path: String,
    /// The file's size in bytes.
    pub bytes: usize,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// What `file.template.list` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TemplateListed {
    /// Every template in the folder, newest first.
    pub templates: Vec<TemplateInfo>,
    /// The folder they are in, so the UI can say where they live.
    pub folder: String,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// What `file.template.delete` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TemplateDeleted {
    /// The template that is gone.
    pub name: String,
    /// The path it was at.
    pub path: String,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// `file.template.list` takes nothing.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TemplateListParams {}

/// `file.template.import { path, name?, overwrite? }`: copy a design file into the templates
/// folder. Refuses a file that is not a design.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TemplateImportParams {
    /// The design file to copy in (.sc or .solidcraft).
    pub path: String,
    /// The template's name. Default: the file's name without its extension.
    #[serde(default)]
    pub name: Option<String>,
    /// true replaces an existing template of that name. Default: false, and the command refuses.
    #[serde(default)]
    pub overwrite: Option<bool>,
}

/// `file.template.export { name, path, overwrite? }`: copy a template out to a chosen path.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TemplateExportParams {
    /// The template's name, as `file.template.list` gives it.
    pub name: String,
    /// Where to write the copy.
    pub path: String,
    /// true replaces a file that is already there. Default: false, and the command refuses.
    #[serde(default)]
    pub overwrite: Option<bool>,
}

/// `file.template.delete { name }`: remove a template. The designs made from it are untouched.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TemplateDeleteParams {
    /// The template's name, as `file.template.list` gives it.
    pub name: String,
}
