pub mod artifacts;
pub mod infra;
pub mod manifest;
pub mod report;
pub mod signature;

use std::path::Path;

use ed25519_dalek::{SigningKey, VerifyingKey};

pub use report::{Check, CheckStatus, InfraDiffEntry, Report, Verdict};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn sign_bundle(_bundle: &Path, _key: &SigningKey) -> Result<(), Error> {
    todo!()
}

pub fn verify(_bundle: &Path, _trusted: &VerifyingKey) -> Report {
    todo!()
}
