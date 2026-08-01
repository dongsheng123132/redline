fn main() {
    println!("{}", serde_json::to_string_pretty(&redline_core::registry_bundle()).expect("Registry bundle is serializable"));
}
