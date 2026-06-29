use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::errors::{Result, SwapError};

// ─── RPC Types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RpcRequest<T> {
    id: u64,
    method: String,
    params: T,
}

#[derive(Deserialize, Debug)]
struct RpcResponse<T> {
    id: serde_json::Value,
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize, Debug)]
struct RpcError {
    code: i64,
    message: String,
}

// ─── Request / Response Structs ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct InitKeyParams {
    pub password: String,
}

#[derive(Serialize)]
pub struct VerifyVtxoParams {
    pub txid: String,
    pub vout: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confirmations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arkd_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub btc_rpc_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct VerifyVtxoResult {
    pub valid: bool,
    pub commitment_txid: String,
    pub vtxo_root_txid: String,
    pub diagnostics: Vec<String>,
}

#[derive(Serialize)]
pub struct OnReceiveVtxoParams {
    pub txid: String,
    pub vout: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arkd_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub btc_rpc_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct OnReceiveVtxoResult {
    pub success: bool,
    pub diagnostics: Vec<String>,
    pub error: Option<String>,
}

// ─── Bridge Client ────────────────────────────────────────────────────────────

/// A JSON-RPC client that spawns and communicates with the Node.js ark-bridge.
pub struct ArkBridge {
    process: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicU64,
}

impl ArkBridge {
    /// Spawns the Node.js bridge process.
    /// Expects `node` to be available in PATH and the `ark-bridge` directory
    /// to be properly configured (e.g., `npm run start`).
    pub fn spawn(bridge_dir: PathBuf) -> Result<Self> {
        let mut child = Command::new("npm")
            .arg("run")
            .arg("start")
            .current_dir(bridge_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Let stderr pass through for logs
            .spawn()
            .map_err(|e| SwapError::Key(format!("Failed to spawn ARK bridge: {}", e)))?;

        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = BufReader::new(child.stdout.take().expect("Failed to open stdout"));

        let bridge = Self {
            process: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(stdout),
            next_id: AtomicU64::new(1),
        };

        Ok(bridge)
    }

    /// Perform a synchronous JSON-RPC call over stdin/stdout.
    fn call<P: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = RpcRequest {
            id,
            method: method.to_string(),
            params,
        };

        let req_json = serde_json::to_string(&req).map_err(SwapError::Json)?;

        // Send request
        {
            let mut stdin = self.stdin.lock().unwrap();
            writeln!(stdin, "{}", req_json).map_err(SwapError::Io)?;
            stdin.flush().map_err(SwapError::Io)?;
        }

        // Read response
        let mut line = String::new();
        {
            let mut stdout = self.stdout.lock().unwrap();
            stdout.read_line(&mut line).map_err(SwapError::Io)?;
        }

        let resp: RpcResponse<R> = serde_json::from_str(&line).map_err(|e| SwapError::Json(e))?;

        if let Some(err) = resp.error {
            return Err(SwapError::Key(format!(
                "ARK bridge error ({}): {}",
                err.code, err.message
            )));
        }

        resp.result
            .ok_or_else(|| SwapError::Key(format!("ARK bridge returned neither result nor error")))
    }

    // ─── API Wrapper ──────────────────────────────────────────────────────────

    pub fn ping(&self) -> Result<()> {
        let _res: serde_json::Value = self.call("ping", serde_json::json!({}))?;
        Ok(())
    }

    pub fn init_key(&self, password: &str) -> Result<()> {
        let params = InitKeyParams {
            password: password.to_string(),
        };
        let _res: serde_json::Value = self.call("initKey", params)?;
        Ok(())
    }

    pub fn verify_vtxo(&self, params: VerifyVtxoParams) -> Result<VerifyVtxoResult> {
        self.call("verifyVtxo", params)
    }

    pub fn on_receive_vtxo(&self, params: OnReceiveVtxoParams) -> Result<OnReceiveVtxoResult> {
        self.call("onReceiveVtxo", params)
    }
}

impl Drop for ArkBridge {
    fn drop(&mut self) {
        if let Ok(mut process) = self.process.lock() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}
