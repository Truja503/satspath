use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Local/mock preview only.
    Preview,
    /// Real mainnet public data is allowed, but execution is not.
    MainnetPreview,
    /// Testnet-only experimental swap/Ark intent gates.
    TestnetExperimental,
    /// Manual execution by the user via a third-party wallet.
    ManualWallet,
}
