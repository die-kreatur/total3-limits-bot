use std::{error::Error, fmt::Display};

pub type Result<T> = std::result::Result<T, ServiceError>;

#[derive(Debug)]
pub enum ServiceError {
    SymbolNotFound(String),
    UnsupportedSymbol(String),
    Unauthorized,
    Internal(String),
}

impl ServiceError {
    pub fn internal(msg: String) -> Self {
        Self::Internal(msg)
    }
}

impl<E: Error> From<E> for ServiceError {
    fn from(value: E) -> Self {
        Self::Internal(value.to_string())
    }
}

impl Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match &self {
            ServiceError::Internal(msg) => msg,
            ServiceError::SymbolNotFound(symbol) => &format!("{} not found", symbol),
            ServiceError::UnsupportedSymbol(symbol) => &format!("{} not supported", symbol),
            ServiceError::Unauthorized => &format!("Action not allowed"),
        };

        write!(f, "{}", val)
    }
}
