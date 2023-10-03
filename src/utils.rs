use md5::{Md5, Digest};

pub(crate) fn latin1_to_string(s: &[u8]) -> String {
    s.iter().map(|&c| c as char).collect()
}

pub(crate) fn bytes_to_hex(bytes: &Vec<u8>) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02X} ", byte))
        .collect::<String>()
}

pub(crate) fn md5_digest(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash_str = String::new();
    for byte in result.iter() {
        hash_str.push_str(&format!("{:02x}", byte));
    }

    hash_str
}