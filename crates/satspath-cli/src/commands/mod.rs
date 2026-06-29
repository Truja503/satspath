mod demo;
mod encode;
mod init;
mod invite;
mod pay;
mod quote;
mod qr;
mod register;
mod show;

pub use demo::cmd_demo;
pub use encode::{cmd_decode, cmd_encode};
pub use init::cmd_init;
pub use invite::cmd_invite;
pub use pay::cmd_pay;
pub use quote::cmd_quote;
pub use register::cmd_register;
pub use show::cmd_show;

use anyhow::Result;
use satspath_core::registry::Registry;
use std::path::PathBuf;

pub(crate) fn satspath_dir() -> PathBuf {
    PathBuf::from(".satspath")
}

pub(crate) fn open_registry() -> Result<Registry> {
    let dir = satspath_dir();
    if !dir.exists() {
        anyhow::bail!(".satspath/ not found. Run `satspath init` first.");
    }
    Ok(Registry::open(&dir)?)
}

use satspath_core::resolver::ChainResolver;
use satspath_core::resolvers::http::HttpResolver;

pub(crate) fn get_resolver() -> Result<ChainResolver> {
    let mut chain = ChainResolver::new();
    
    // Add local registry first
    if let Ok(reg) = open_registry() {
        chain = chain.push(reg);
    }
    
    // Add public HTTP resolver fallback
    chain = chain.push(HttpResolver::new());
    
    Ok(chain)
}
