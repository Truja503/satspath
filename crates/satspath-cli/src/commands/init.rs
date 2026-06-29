use anyhow::Result;
use serde_json::json;
use std::fs;

use super::satspath_dir;

pub fn cmd_init() -> Result<()> {
    let dir = satspath_dir();
    fs::create_dir_all(&dir)?;

    let registry_path = dir.join("registry.json");
    if !registry_path.exists() {
        fs::write(&registry_path, "{\"profiles\":{}}")?;
        println!("Created {}", registry_path.display());
    } else {
        println!("Registry already exists at {}", registry_path.display());
    }

    let keys_path = dir.join("keys.json");
    if !keys_path.exists() {
        let placeholder = json!({
            "warning": "This file stores DEMO keys only. Never use real funds.",
            "keys": {}
        });
        fs::write(&keys_path, serde_json::to_string_pretty(&placeholder)?)?;
        println!("Created {}", keys_path.display());
    } else {
        println!("Keys file already exists at {}", keys_path.display());
    }

    println!();
    println!("SatsPath initialized at {}/", dir.display());
    println!("NOTE: .satspath/ is git-ignored and stays local to this machine.");
    Ok(())
}
