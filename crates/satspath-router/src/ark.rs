use satspath_core::PaymentMethod;

/// Adapter trait for an Ark server client.
pub trait ArkClient {
    fn is_available(&self) -> bool;
    fn create_payment_intent(&self, amount_sats: u64, pubkey: &str) -> anyhow::Result<String>;
}

/// Mock Ark client used in the prototype.
pub struct MockArkClient {
    pub available: bool,
}

impl ArkClient for MockArkClient {
    fn is_available(&self) -> bool {
        self.available
    }

    fn create_payment_intent(&self, amount_sats: u64, pubkey: &str) -> anyhow::Result<String> {
        if !self.available {
            anyhow::bail!("Ark server unavailable");
        }
        Ok(format!(
            "ark:intent:mock:{}:{}",
            pubkey.chars().take(8).collect::<String>(),
            amount_sats
        ))
    }
}

/// Check whether any Ark method exists in a method list.
pub fn is_ark_available(methods: &[PaymentMethod]) -> bool {
    methods
        .iter()
        .any(|m| matches!(m, PaymentMethod::Ark { .. }))
}

/// Find the first Ark method.
pub fn first_ark_method(methods: &[PaymentMethod]) -> Option<&PaymentMethod> {
    methods
        .iter()
        .find(|m| matches!(m, PaymentMethod::Ark { .. }))
}
