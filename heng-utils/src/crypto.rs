use hmac::{Hmac, Mac, NewMac};
use sha2::{Digest, Sha256};

pub fn to_hex_string(src: &[u8]) -> String {
    faster_hex::hex_string(src).unwrap()
}

pub fn hex_sha256(data: &[u8]) -> String {
    let src = Sha256::digest(data);
    to_hex_string(src.as_ref())
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> impl AsRef<[u8]> {
    let mut m = <Hmac<Sha256>>::new_varkey(key).unwrap();
    m.update(data.as_ref());
    m.finalize().into_bytes()
}

pub fn hex_hmac_sha256(key: &[u8], data: &[u8]) -> String {
    let src = hmac_sha256(key, data);
    to_hex_string(src.as_ref())
}
