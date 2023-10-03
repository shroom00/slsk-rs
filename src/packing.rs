use std::net::Ipv4Addr;

pub trait PackToBytes: Sized {
    fn pack_to_bytes(&self) -> Vec<u8>;
}

/// Used for specifying a `Message` that doesn't get sent
pub struct IsntSent;

impl PackToBytes for IsntSent {
    fn pack_to_bytes(&self) -> Vec<u8> {
        vec![]
    }
}

impl PackToBytes for i8 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

impl PackToBytes for i32 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl PackToBytes for u32 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl PackToBytes for i64 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl PackToBytes for u64 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl PackToBytes for bool {
    fn pack_to_bytes(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

impl PackToBytes for String {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut packed_string = vec![0, 0, 0, 0];
        packed_string.extend(self.as_bytes());
        let length = (self.len() as u32).pack_to_bytes();
        packed_string.splice(..4, length);
        packed_string
    }
}

pub trait UnpackFromBytes: Sized {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self;
    /// Used for when data can only be unpacked based on self
    /// Useful for enums
    fn _unpack_self_from_bytes(&self, bytes: &mut Vec<u8>) -> Self {
        Self::unpack_from_bytes(bytes)
    }
}

// Used for specifying a `Message` that doesn't get received
pub struct IsntReceived;

impl UnpackFromBytes for IsntReceived {
    fn unpack_from_bytes(_: &mut Vec<u8>) -> Self {
        IsntReceived
    }
}

impl UnpackFromBytes for bool {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let buf: Vec<u8> = bytes.drain(..1).collect();
        buf[0] != 0
    }
}

impl UnpackFromBytes for u8 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let buf: Vec<u8> = bytes.drain(..1).collect();
        buf[0]
    }
}

impl UnpackFromBytes for i32 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let mut buf: [u8; 4] = [0, 0, 0, 0];
        let temp_buf: Vec<u8> = bytes.drain(..4).collect();
        buf.copy_from_slice(&temp_buf);
        i32::from_le_bytes(buf)
    }
}

impl UnpackFromBytes for u32 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let mut buf: [u8; 4] = [0, 0, 0, 0];
        let temp_buf: Vec<u8> = bytes.drain(..4).collect();
        buf.copy_from_slice(&temp_buf);
        u32::from_le_bytes(buf)
    }
}
impl UnpackFromBytes for u64 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let mut buf: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let temp_buf: Vec<u8> = bytes.drain(..8).collect();
        buf.copy_from_slice(&temp_buf);
        u64::from_le_bytes(buf)
    }
}

impl UnpackFromBytes for String {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let length = <u32>::unpack_from_bytes(bytes);
        let mut buf: Vec<u8> = vec![0; length.try_into().unwrap()];
        let temp_buf: Vec<u8> = bytes.drain(..(length as usize)).collect();
        buf.copy_from_slice(&temp_buf);
        String::from_utf8_lossy(&mut buf).to_string()
    }
}

impl UnpackFromBytes for Ipv4Addr {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Self {
        let combined_octets = u32::unpack_from_bytes(bytes);
        Ipv4Addr::from(combined_octets)
    }
}
