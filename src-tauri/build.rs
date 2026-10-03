use std::fs;
use std::path::Path;

fn main() {
    embed_device_defaults();
    tauri_build::build();
}

fn embed_device_defaults() {
    let path = Path::new("embedded_device.local.json");
    println!("cargo:rerun-if-changed=embedded_device.local.json");
    println!("cargo:rerun-if-env-changed=NIGHTDESK_PILOT_BUILD");
    let pilot = std::env::var("NIGHTDESK_PILOT_BUILD").as_deref() == Ok("1");
    if pilot {
        println!("cargo:warning=Nightdesk LAN pilot: cloud credentials are not embedded");
    }
    let parsed = if pilot {
        serde_json::Value::Null
    } else if path.is_file() {
        let raw = fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };
    let version = parsed["remote_auth_version"]
        .as_i64()
        .or_else(|| parsed["remote_auth_version"].as_u64().map(|n| n as i64))
        .or_else(|| {
            parsed["remote_auth_version"]
                .as_str()
                .and_then(|s| s.trim().parse().ok())
        })
        .unwrap_or(0)
        .max(0);
    let pairs = [
        (
            "NIGHTDESK_DEVICE_URL",
            parsed["project_url"].as_str().unwrap_or("").trim().to_string(),
        ),
        (
            "NIGHTDESK_DEVICE_ANON",
            parsed["anon_key"].as_str().unwrap_or("").trim().to_string(),
        ),
        (
            "NIGHTDESK_DEVICE_EMAIL",
            parsed["device_email"].as_str().unwrap_or("").trim().to_string(),
        ),
        (
            "NIGHTDESK_DEVICE_PASSWORD",
            parsed["device_password"].as_str().unwrap_or("").to_string(),
        ),
        (
            "NIGHTDESK_REMOTE_EMAIL",
            parsed["remote_email"].as_str().unwrap_or("").trim().to_string(),
        ),
        (
            "NIGHTDESK_REMOTE_PASSWORD",
            parsed["remote_password"].as_str().unwrap_or("").to_string(),
        ),
        ("NIGHTDESK_REMOTE_AUTH_VERSION", version.to_string()),
    ];
    for (key, value) in pairs {
        if value.bytes().any(|b| b == b'\n' || b == b'\r' || b == 0) {
            panic!("{key} no puede tener saltos de línea");
        }
        println!("cargo:rustc-env={key}={value}");
    }
}
