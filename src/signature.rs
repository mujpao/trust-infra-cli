use crate::report::{Check, fail, pass};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde_json::Value;

pub fn sign_value(manifest: &mut Value, key: &SigningKey) {
    let obj = manifest
        .as_object_mut()
        .expect("manifest must be an object");
    obj.remove("signature");
    let bytes = serde_jcs::to_vec(obj).expect("should be able to canonicalize manifest json");
    let signature = key.sign(&bytes);

    obj.insert(
        "signature".into(),
        Value::String(STANDARD.encode(signature.to_bytes())),
    );
}

pub fn check_signature(value: &Value, trusted: &VerifyingKey) -> Check {
    match verify_signature_inner(value, trusted) {
        Ok(()) => pass("signature", "valid for pinned key"),
        Err(msg) => fail("signature", msg),
    }
}

fn verify_signature_inner(value: &Value, trusted: &VerifyingKey) -> Result<(), String> {
    let obj = value.as_object().ok_or("manifest is not a JSON object")?;
    let sig_b64 = obj
        .get("signature")
        .and_then(Value::as_str)
        .ok_or("missing or non-string signature field")?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_b64)
        .map_err(|e| format!("bad base64: {e}"))?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "signature is not 64 bytes".to_string())?;
    let sig = Signature::from_bytes(&sig_arr);

    let mut rest = obj.clone();
    rest.remove("signature");
    let msg = serde_jcs::to_vec(&Value::Object(rest))
        .map_err(|e| format!("canonicalization failed: {e}"))?;

    trusted
        .verify_strict(&msg, &sig)
        .map_err(|_| "signature does not match pinned key".to_string())
}
