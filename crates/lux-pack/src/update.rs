//! Verify a signed update blob. The bytes come from disk or a caller;
//! this module never downloads.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("bad public key")]
    BadKey,
    #[error("signature does not match payload")]
    VerifyFailed,
}

/// `public_key` and `signature` are 32- and 64-byte Ed25519 values.
pub fn verify_update(
    public_key: &[u8; 32],
    signature: &[u8; 64],
    payload: &[u8],
) -> Result<(), UpdateError> {
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| UpdateError::BadKey)?;
    let sig = Signature::from_bytes(signature);
    key.verify(payload, &sig)
        .map_err(|_| UpdateError::VerifyFailed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn accepts_a_real_signature_and_rejects_tampering() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let payload = b"lux-0.9.1";
        let sig = signing.sign(payload);
        let vk = signing.verifying_key();
        verify_update(vk.as_bytes(), &sig.to_bytes(), payload).unwrap();
        assert!(verify_update(vk.as_bytes(), &sig.to_bytes(), b"tampered").is_err());
    }
}
