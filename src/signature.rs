use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
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
