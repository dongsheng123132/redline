//! Redline's executable Action Registry and only Action-to-handler map.
//!
//! Each registration owns the Action contract and executable handler. Manifest,
//! CLI, GUI constants, evidence bindings, and runtime dispatch derive from it.

use std::sync::{Arc, OnceLock};

use action_parity_core::{
    ActionDescriptor, ActionError, Application, Confirmation, EffectClass, Effects, Reachability, Registry, Risk, Surface, SurfaceKind,
};
use serde_json::{json, Value};

use crate::error::RedlineError;

/// Stable public Action IDs live beside their executable registrations.
pub mod id {
    pub const INSPECT: &str = "document.inspect";
    pub const DIFF: &str = "document.diff";
    pub const APPLY: &str = "document.apply-track-changes";
    pub const VERIFY: &str = "document.verify";
    pub const FORMATS: &str = "document.formats";
    pub const ARCHIVE_LIST: &str = "archive.list";
    pub const ARCHIVE_EXTRACT: &str = "archive.extract";
    pub const AGENT_DISPATCH: &str = "agent.dispatch";
    pub const AGENT_CATALOG: &str = "agent.catalog";
}

pub fn get() -> Arc<Registry> {
    static REGISTRY: OnceLock<Arc<Registry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(build())))
}

pub fn bundle() -> Value {
    get().artifact_bundle()
}

pub fn catalog() -> Vec<Value> {
    get().manifest()["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            json!({
                "id": item["id"],
                "description": item["description"],
            })
        })
        .collect()
}

fn build() -> Registry {
    let mut registry = Registry::new(Application {
        id: "org.redline.desktop".into(),
        name: "Redline".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: Some("Universal document preview, annotation, diff, and AI handoff layer.".into()),
        homepage: None,
        source: Some("https://github.com/dongsheng123132/redline".into()),
    })
    .generator_revision("crates/redline-core/src/registry.rs");

    let mut cli = Surface::new("cli", SurfaceKind::Cli, Reachability::External, "redline call {action_id} --params <json> --json");
    cli.binding_test = Some("tests/action-parity-bindings.test.mjs".into());
    cli.test_driver = Some("node:test + redline-cli".into());
    registry.add_surface(cli).expect("CLI Surface is static and valid");
    let mut gui = Surface::new("gui", SurfaceKind::Gui, Reachability::InProcess, "generated:ACTION[{action_id}]");
    gui.required_for_parity = false;
    gui.binding_test = Some("tests/action-parity-bindings.test.mjs".into());
    gui.test_driver = Some("node:test + action-parity-tauri".into());
    gui.exclusion_reason = Some(
        "Gradual migration: this Surface contains only Actions currently called by the desktop GUI; CLI remains the complete machine Surface."
            .into(),
    );
    registry.add_surface(gui).expect("GUI Surface is static and valid");

    register(
        &mut registry,
        id::INSPECT,
        "Inspect document",
        "Parse a local file into a source-hash-bound ShadowDoc projection.",
        object(&["path"], &[("path", string())]),
        super::inspect_action,
        Effects::read_only(),
        true,
    );
    register(
        &mut registry,
        id::DIFF,
        "Compare documents",
        "Compare two same-format document projections without changing either file.",
        object(&["before", "after"], &[("before", string()), ("after", string())]),
        super::diff_action,
        Effects::read_only(),
        true,
    );
    register(
        &mut registry,
        id::APPLY,
        "Apply tracked changes",
        "Write an explicit patch to a new DOCX with native Track Changes; never overwrite the source.",
        object(
            &["input", "patch", "output"],
            &[("input", string()), ("patch", json!({"type":"object"})), ("output", string()), ("author", string()), ("force", boolean())],
        ),
        super::apply_action,
        Effects::write(true),
        false,
    );
    register(
        &mut registry,
        id::VERIFY,
        "Verify document",
        "Check whether a local file can be parsed without changing it.",
        object(&["path"], &[("path", string())]),
        super::verify_action,
        Effects::read_only(),
        false,
    );
    register(
        &mut registry,
        id::FORMATS,
        "List formats",
        "Return the canonical format and capability registry.",
        object(&[], &[]),
        super::formats_action,
        Effects::read_only(),
        false,
    );
    register(
        &mut registry,
        id::ARCHIVE_LIST,
        "List archive",
        "List archive entries without extracting files.",
        object(&["path"], &[("path", string())]),
        super::archive_list_action,
        Effects::read_only(),
        false,
    );
    register(
        &mut registry,
        id::ARCHIVE_EXTRACT,
        "Extract archive",
        "Extract an archive into an explicit destination without changing the archive.",
        object(&["path", "dest"], &[("path", string()), ("dest", string()), ("overwrite", boolean())]),
        super::archive_extract_action,
        Effects::write(true),
        false,
    );
    register(
        &mut registry,
        id::AGENT_CATALOG,
        "List AI agents",
        "Return supported AI coding agents and their local availability.",
        object(&[], &[]),
        super::agent_catalog_action,
        Effects::read_only(),
        true,
    );
    register(
        &mut registry,
        id::AGENT_DISPATCH,
        "Dispatch AI agent",
        "Send annotations and an immutable source reference to an AI agent that writes a new output file.",
        object(
            &["agent", "source", "output"],
            &[
                ("agent", string()),
                ("source", string()),
                ("expectedSourceSha256", string()),
                ("output", string()),
                ("annotations", json!({"type":"array","items":{"type":"object"}})),
                ("instruction", string()),
                ("cwd", string()),
                ("timeoutSecs", json!({"type":"integer","minimum":0})),
                ("dryRun", boolean()),
            ],
        ),
        super::agent_dispatch_action,
        Effects {
            effect_class: EffectClass::External,
            risk: Risk::Medium,
            reversible: true,
            confirmation: Confirmation::Conditional,
            audit_required: true,
            rollback_action: None,
            notes: Some("The agent writes a new output path; Redline never modifies the source.".into()),
        },
        true,
    );

    registry
}

type Handler = fn(&Value) -> crate::error::Result<Value>;

fn register(
    registry: &mut Registry,
    id: &'static str,
    title: &str,
    description: &str,
    input_schema: Value,
    handler: Handler,
    effects: Effects,
    gui: bool,
) {
    let mut descriptor = ActionDescriptor::new(id, title, description, input_schema, json!({"type":"object"}), effects).surface("cli");
    descriptor.execution.headless_evidence = Some("cargo test -p redline-core".into());
    if gui {
        descriptor = descriptor.surface("gui");
    }
    registry
        .register(descriptor, move |_, input| handler(&input).map_err(map_error))
        .unwrap_or_else(|error| panic!("invalid Redline Action {id}: {error}"));
}

fn map_error(error: RedlineError) -> ActionError {
    ActionError { class: error.class.as_str().into(), code: error.code.into(), message: error.message, details: error.details }
}

fn object(required: &[&str], properties: &[(&str, Value)]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
            .iter()
            .map(|(name, schema)| ((*name).to_string(), schema.clone()))
            .collect::<serde_json::Map<String, Value>>(),
    })
}

fn string() -> Value {
    json!({"type":"string"})
}

fn boolean() -> Value {
    json!({"type":"boolean"})
}

#[cfg(test)]
mod tests {
    use super::*;
    use action_parity_core::DispatchRequest;

    #[test]
    fn registry_has_nine_actions_without_fake_mcp_tools() {
        let bundle = bundle();
        assert_eq!(bundle["manifest"]["actions"].as_array().unwrap().len(), 9);
        assert_eq!(bundle["cli_help"]["actions"].as_array().unwrap().len(), 9);
        assert!(bundle["mcp_tools"]["tools"].as_array().unwrap().is_empty());
    }

    #[test]
    fn gui_surface_contains_only_actions_the_frontend_calls() {
        let manifest = get().manifest();
        let gui_actions = manifest["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| action["bindings"].as_array().unwrap().iter().any(|binding| binding["surface"] == "gui"))
            .map(|action| action["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(gui_actions, vec!["agent.catalog", "agent.dispatch", "document.diff", "document.inspect"]);
    }

    #[test]
    fn registry_adapter_reaches_existing_handler_and_preserves_execution_id() {
        let output = get().dispatch(DispatchRequest {
            action_id: id::FORMATS.into(),
            input: json!({}),
            confirmed: false,
            execution_id: Some("redline-registry-test".into()),
            surface: Some("cli".into()),
        });
        assert!(output.ok);
        assert_eq!(output.execution_id, "redline-registry-test");
        assert!(output.result.unwrap()["formats"].is_array());
    }
}
