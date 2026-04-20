use std::fs;
use std::path::{Path, PathBuf};

mod compile;
mod schema;

pub use compile::{compile_template, CompiledTemplate};
pub use schema::{
    CompileBinding, ControlTemplate, InputField, TemplateCompile, TemplateDefaults,
    TemplateMetadata, TemplateValidationError,
};

pub struct TemplateModuleMarker;

pub fn parse_template_yaml(yaml: &str) -> Result<ControlTemplate, TemplateValidationError> {
    let template: ControlTemplate = serde_yaml::from_str(yaml)
        .map_err(|err| TemplateValidationError::new(format!("invalid template yaml: {err}")))?;
    template.validate()?;
    Ok(template)
}

pub fn list_templates() -> Result<Vec<TemplateMetadata>, TemplateValidationError> {
    let mut templates = Vec::new();
    let template_dir = template_dir();
    let entries = fs::read_dir(&template_dir).map_err(|err| {
        TemplateValidationError::new(format!(
            "failed to read template dir '{}': {err}",
            template_dir.display()
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| {
            TemplateValidationError::new(format!(
                "failed to read template dir entry '{}': {err}",
                template_dir.display()
            ))
        })?;
        let path = entry.path();
        if !is_yaml_file(&path) {
            continue;
        }
        let template = load_template_from_path(&path)?;
        templates.push(template.template);
    }

    templates.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(templates)
}

pub fn load_template(id: &str) -> Result<ControlTemplate, TemplateValidationError> {
    let path = template_dir().join(format!("{id}.yaml"));
    load_template_from_path(&path)
}

fn load_template_from_path(path: &Path) -> Result<ControlTemplate, TemplateValidationError> {
    let yaml = fs::read_to_string(path).map_err(|err| {
        TemplateValidationError::new(format!(
            "failed to read template file '{}': {err}",
            path.display()
        ))
    })?;
    parse_template_yaml(&yaml)
}

fn template_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates")
}

fn is_yaml_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("yaml"))
}
