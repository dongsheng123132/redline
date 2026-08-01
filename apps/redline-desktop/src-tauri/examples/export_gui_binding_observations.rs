//! Headless evidence driver for the real Redline Tauri adapter.
//!
//! It derives the GUI Action set from the Registry, calls the same thin adapter
//! managed by the desktop app, and prints verifier observations as JSON.

use action_parity_tauri::{DispatchRequest, TauriAdapter};
use serde_json::{json, Value};

fn main() {
    let registry = redline_core::action_registry();
    let manifest = registry.manifest();
    let adapter = TauriAdapter::from_shared(registry);
    let mut observations = Vec::new();

    for action in manifest["actions"].as_array().into_iter().flatten() {
        let exposed = action["bindings"].as_array().into_iter().flatten().any(|binding| binding["surface"] == "gui");
        if !exposed {
            continue;
        }
        let action_id = action["id"].as_str().expect("generated Action ID");
        let execution_id = format!("evidence-gui-{action_id}");
        let output = adapter.call(DispatchRequest {
            action_id: action_id.into(),
            input: Value::Object(Default::default()),
            confirmed: true,
            execution_id: Some(execution_id.clone()),
            surface: Some("gui".into()),
        });
        observations.push(json!({
            "action_id": action_id,
            "surface": "gui",
            "request_execution_id": execution_id,
            "core_execution_id": output["execution_id"],
        }));
    }

    println!("{}", serde_json::to_string(&observations).expect("observations serialize"));
}
