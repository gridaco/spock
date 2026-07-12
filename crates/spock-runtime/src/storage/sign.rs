//! Native HMAC signed URLs (RFD 0018 §1.4) — the offline-verifiable,
//! method-bound, time-limited construction of an S3 presigned URL, with the
//! runtime's own per-run secret in place of SigV4. The signature binds the
//! HTTP method, the object id, and the expiry, and nothing else (there is no
//! host or bucket surface to bind in v0).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// A per-run signing secret. Ephemeral: minted at startup, gone on exit —
/// coherent with the disposable database, since a URL cannot outlive the
/// objects it names (a restart wipes both).
pub struct Signer {
    key: [u8; 32],
}

impl Signer {
    /// 32 bytes of OS entropy.
    pub fn random() -> Signer {
        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key).expect("OS entropy for the storage signing key");
        Signer { key }
    }

    fn mac(&self, method: &str, id: &str, exp: i64) -> HmacSha256 {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC accepts any key length");
        mac.update(method.as_bytes());
        mac.update(b"\n");
        mac.update(id.as_bytes());
        mac.update(b"\n");
        mac.update(exp.to_string().as_bytes());
        mac
    }

    /// A relative signed URL bound to `method` for object `id`, expiring
    /// `ttl_secs` after `now` (both unix seconds).
    pub fn url(&self, method: &str, id: &str, now: i64, ttl_secs: i64) -> String {
        let exp = now + ttl_secs;
        let sig = hex::encode(self.mac(method, id, exp).finalize().into_bytes());
        format!("/storage/v1/object/{id}?exp={exp}&sig={sig}")
    }

    /// True iff `sig_hex` is this signer's tag over (method, id, exp).
    /// Constant-time on the tag (`verify_slice`, never a string `==`); expiry
    /// is the caller's separate check.
    pub fn verify(&self, method: &str, id: &str, exp: i64, sig_hex: &str) -> bool {
        let Ok(sig) = hex::decode(sig_hex) else {
            return false;
        };
        self.mac(method, id, exp).verify_slice(&sig).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_tampering() {
        let s = Signer::random();
        let now = 1_000_000i64;
        let url = s.url("PUT", "abc", now, 600);
        // parse exp out of the url we just minted
        let exp: i64 = url
            .split("exp=")
            .nth(1)
            .and_then(|q| q.split('&').next())
            .and_then(|v| v.parse().ok())
            .unwrap();
        let sig = url.split("sig=").nth(1).unwrap().to_string();

        assert!(
            s.verify("PUT", "abc", exp, &sig),
            "valid signature verifies"
        );
        assert!(!s.verify("GET", "abc", exp, &sig), "method is bound");
        assert!(!s.verify("PUT", "xyz", exp, &sig), "object id is bound");
        assert!(!s.verify("PUT", "abc", exp + 1, &sig), "expiry is bound");
        assert!(!s.verify("PUT", "abc", exp, "deadbeef"), "wrong sig fails");
        assert!(!s.verify("PUT", "abc", exp, "not-hex!"), "non-hex fails");

        // a different secret never validates this signature
        assert!(!Signer::random().verify("PUT", "abc", exp, &sig));
    }
}
