use hkdf::Hkdf;
use libp2p::PeerId;
use sha2::Sha256;

use super::ProxyCredentials;

/// Relay 端：基于 peer_id 派生确定性凭据
/// HKDF-SHA256, salt=secret, ikm=peer_id.to_bytes()
pub fn derive_credentials(peer_id: &PeerId, secret: &[u8]) -> ProxyCredentials {
    let hk = Hkdf::<Sha256>::new(Some(secret), peer_id.to_bytes().as_slice());

    let mut username_bytes = [0u8; 8];
    hk.expand(b"proxy-username", &mut username_bytes)
        .expect("8 bytes is a valid HKDF-SHA256 output length");

    let mut password_bytes = [0u8; 16];
    hk.expand(b"proxy-password", &mut password_bytes)
        .expect("16 bytes is a valid HKDF-SHA256 output length");

    ProxyCredentials {
        username: hex::encode(username_bytes),
        password: hex::encode(password_bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic() {
        let peer_id = PeerId::random();
        let secret = b"test-secret";
        let c1 = derive_credentials(&peer_id, secret);
        let c2 = derive_credentials(&peer_id, secret);
        assert_eq!(c1.username, c2.username);
        assert_eq!(c1.password, c2.password);
        assert_eq!(c1.username.len(), 16);
        assert_eq!(c1.password.len(), 32);
    }

    #[test]
    fn different_peers_get_different_credentials() {
        let secret = b"test-secret";
        let c1 = derive_credentials(&PeerId::random(), secret);
        let c2 = derive_credentials(&PeerId::random(), secret);
        assert_ne!(c1.username, c2.username);
        assert_ne!(c1.password, c2.password);
    }
}
