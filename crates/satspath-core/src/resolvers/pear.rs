use async_trait::async_trait;
use std::process::Command;
use std::env;
use std::path::PathBuf;

use crate::resolver::ProfileResolver;
use crate::{Result, SatsPathError, SignedPaymentProfile};

pub struct PearResolver {
    pear_script_path: PathBuf,
}

impl Default for PearResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PearResolver {
    pub fn new() -> Self {
        // Find satspath-pear/index.js relative to the current working directory,
        // or a configured path. For prototype, we assume it's in the repo root.
        let mut path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push("satspath-pear");
        path.push("index.js");
        
        Self {
            pear_script_path: path,
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            pear_script_path: path,
        }
    }
}

#[async_trait]
impl ProfileResolver for PearResolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        if !self.pear_script_path.exists() {
            // If the JS script isn't found, fail silently for the chain resolver
            return Err(SatsPathError::AliasNotFound(alias.to_string()));
        }

        // Spawn `node satspath-pear/index.js resolve <alias>`
        let output = Command::new("node")
            .arg(&self.pear_script_path)
            .arg("resolve")
            .arg(alias)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(_) => return Err(SatsPathError::AliasNotFound(alias.to_string())),
        };

        if !output.status.success() {
            return Err(SatsPathError::AliasNotFound(alias.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse the JSON output as a SignedPaymentProfile
        match serde_json::from_str::<SignedPaymentProfile>(stdout.trim()) {
            Ok(profile) => {
                // SEC-01: enforce profile expiry before returning to the router
                if let Err(_) = crate::crypto::check_profile_expiry(&profile.profile) {
                    return Err(SatsPathError::AliasNotFound(alias.to_string()));
                }
                Ok(profile)
            },
            Err(_) => Err(SatsPathError::AliasNotFound(alias.to_string())),
        }
    }
}
