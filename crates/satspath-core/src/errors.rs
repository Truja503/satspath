use thiserror::Error;

#[derive(Debug, Error)]
pub enum SatsPathError {
    #[error("alias not found: {0}")]
    AliasNotFound(String),

    #[error("alias already registered: {0}")]
    AliasAlreadyRegistered(String),

    #[error("invalid signature")]
    InvalidSignature,

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("invalid payment URI: {0}")]
    InvalidPaymentUri(String),

    #[error("no payment methods available")]
    NoPaymentMethods,

    #[error("no suitable payment rail found: {0}")]
    NoRouteFound(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("registry error: {0}")]
    RegistryError(String),

    #[error("crypto error: {0}")]
    CryptoError(String),

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("invalid route: {0}")]
    InvalidRoute(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SatsPathError>;
