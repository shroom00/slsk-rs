use std::net::Ipv4Addr;

pub trait PackToBytes: Sized {
    fn pack_to_bytes(&self) -> Vec<u8>;
}

/// Used for specifying a `Message` that doesn't get sent
pub struct IsntSent;

impl PackToBytes for IsntSent {
    fn pack_to_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}

impl PackToBytes for bool {
    fn pack_to_bytes(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

impl PackToBytes for u8 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        vec![*self]
    }
}

impl PackToBytes for u16 {
    fn pack_to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
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

impl PackToBytes for String {
    fn pack_to_bytes(&self) -> Vec<u8> {
        let mut packed_string = vec![0, 0, 0, 0];
        packed_string.extend(self.as_bytes());
        let length = (self.len() as u32).pack_to_bytes();
        packed_string.splice(..4, length);
        packed_string
    }
}

impl<T> PackToBytes for Vec<T>
where
    T: PackToBytes,
{
    fn pack_to_bytes(&self) -> Vec<u8> {
        let length: u32 = self.len().try_into().unwrap_or(u32::MAX);
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend(length.pack_to_bytes());
        let count: u32 = 1;
        for i in self {
            bytes.extend(i.pack_to_bytes());
            if count == u32::MAX {
                break;
            }
        }
        bytes
    }
}

impl<T> PackToBytes for Option<T>
where
    T: PackToBytes,
{
    fn pack_to_bytes(&self) -> Vec<u8> {
        match self {
            Some(thing) => thing.pack_to_bytes(),
            None => Vec::new(),
        }
    }
}

pub trait UnpackFromBytes: Sized {
    /// Internally, this uses `drain` and so can panic when there aren't enough bytes to unpack required attributes.
    /// In this case, `bytes` gets drained completely and `None` is returned.
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self>;
    /// Used for when data can only be unpacked based on self
    /// Useful for enums
    fn _unpack_self_from_bytes(&self, bytes: &mut Vec<u8>) -> Option<Self> {
        Self::unpack_from_bytes(bytes)
    }
}

// Used for specifying a `Message` that doesn't get received
pub struct IsntReceived;

impl UnpackFromBytes for IsntReceived {
    fn unpack_from_bytes(_: &mut Vec<u8>) -> Option<Self> {
        Some(IsntReceived)
    }
}

impl UnpackFromBytes for bool {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        if bytes.len() == 0 {
            None
        } else {
            let buf: Vec<u8> = bytes.drain(..1).collect();
            Some(buf[0] != 0)
        }
    }
}

impl UnpackFromBytes for u8 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        if bytes.len() == 0 {
            None
        } else {
            let buf: Vec<u8> = bytes.drain(..1).collect();
            Some(buf[0])
        }
    }
}

impl UnpackFromBytes for u16 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {

        if bytes.len() < 2 {
            bytes.drain(..);
            None
        } else {
            let mut buf: [u8; 2] = [0, 0];
            let temp_buf: Vec<u8> = bytes.drain(..2).collect();
            buf.copy_from_slice(&temp_buf);
            Some(u16::from_le_bytes(buf))
        }
    }
}

impl UnpackFromBytes for i32 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        if bytes.len() < 4 {
            bytes.drain(..);
            None
        } else {
            let mut buf: [u8; 4] = [0, 0, 0, 0];
            let temp_buf: Vec<u8> = bytes.drain(..4).collect();
            buf.copy_from_slice(&temp_buf);
            Some(i32::from_le_bytes(buf))
        }
    }
}

impl UnpackFromBytes for u32 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        if bytes.len() < 4 {
            bytes.drain(..);
            None
        } else {
            let mut buf: [u8; 4] = [0, 0, 0, 0];
            let temp_buf: Vec<u8> = bytes.drain(..4).collect();
            buf.copy_from_slice(&temp_buf);
            Some(u32::from_le_bytes(buf))
        }
    }
}
impl UnpackFromBytes for u64 {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        if bytes.len() < 8 {
            bytes.drain(..);
            None
        } else {
            let mut buf: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
            let temp_buf: Vec<u8> = bytes.drain(..8).collect();
            buf.copy_from_slice(&temp_buf);
            Some(u64::from_le_bytes(buf))
        }
    }
}

impl UnpackFromBytes for String {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let length = <u32>::unpack_from_bytes(bytes)?;
        let mut buf: Vec<u8> = vec![0; length.try_into().unwrap()];
        if bytes.len() < length as usize {
            bytes.drain(..);
            None
        } else {
            let temp_buf: Vec<u8> = bytes.drain(..(length as usize)).collect();
            buf.copy_from_slice(&temp_buf);
            Some(String::from_utf8_lossy(&mut buf).to_string())
        }
    }
}

impl UnpackFromBytes for Ipv4Addr {
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let combined_octets = u32::unpack_from_bytes(bytes)?;
        Some(Ipv4Addr::from(combined_octets))
    }
}

impl<T> UnpackFromBytes for Vec<T>
where
    T: UnpackFromBytes,
{
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        let length = <u32>::unpack_from_bytes(bytes)?;
        let mut vec = Vec::new();
        for _ in 0..length {
            vec.push(<T>::unpack_from_bytes(bytes)?);
        }
        Some(vec)
    }
}

impl<T> UnpackFromBytes for Option<T>
where
    T: UnpackFromBytes,
{
    fn unpack_from_bytes(bytes: &mut Vec<u8>) -> Option<Self> {
        if bytes.is_empty() {
            None
        } else {
            Some(<T>::unpack_from_bytes(bytes))
        }
    }
}
