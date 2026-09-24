use std::fs;
use std::path::Path;

fn main() {
    embed_device_defaults();
    tauri_build::build();
}

fn embed_device_defaults() {
    let path = Path::new("embedded_device.local.json");
    println!("cargo:rerun-if-changed=embedded_device.local.json");
    let keys = [
        "NIGHTDESK_DEVICE_URL",
        "NIGHTDESK_DEVICE_ANON",
        "NIGHTDESK_DEVICE_EMAIL",
        "NIGHTDESK_DEVICE_PASSWORD",
    ];
    let values = if path.is_file() {
        let raw = fs::read_to_string(path).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
        [
            parsed["project_url"].as_str().unwrap_or("").trim().to_string(),
            parsed["anon_key"].as_str().unwrap_or("").trim().to_string(),
            parsed["device_email"].as_str().unwrap_or("").trim().to_string(),
            parsed["device_password"].as_str().unwrap_or("").to_string(),
        ]
    } else {
        [String::new(), String::new(), String::new(), String::new()]
    };
    for (key, value) in keys.iter().zip(values.iter()) {
        if value.bytes().any(|b| b == b'\n' || b == b'\r' || b == 0) {
            panic!("{key} no puede tener saltos de línea");
        }
        println!("cargo:rustc-env={key}={value}");
    }
}
