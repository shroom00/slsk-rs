pub(crate) use super::MessageTrait;
pub(crate) use crate::packing::PackToBytes;

macro_rules! generate_struct {
    (
        $struct_name:ident {
            $(
                $field_name:ident: $field_type:ty $({
                    $($iter_name:ident: $iter_type:ty,)+
                })?
            ,)*
        }
    ) => {
        // Debug isn't always used
        #[allow(dead_code)]
        #[derive(Debug)]
        pub struct $struct_name {
            $(
                pub $field_name: $field_type,
            )*
        }
    };
}

macro_rules! impl_pack_to_bytes {
    (
        $struct_name:ident {
            $(
                $field_name:ident: $field_type:ty $(
                    {
                        $($iter_name:ident: $iter_type:ty,)+
                    }
                )?
            ,)*
        }
    ) => {
        impl PackToBytes for $struct_name {
            fn pack_to_bytes(&self) -> Vec<u8> {
                // In the case where there are no fields, it's unused_mut, but if there are fields it needs to be mutable
                #[allow(unused_mut)]
                let mut bytes: Vec<u8> = vec![];
                $(
                    bytes.extend(self.$field_name.pack_to_bytes());
                    $(
                            $(
                                if (self.$iter_name.len() as $field_type) != self.$field_name {
                                    panic!("{} should be the length specified by {} ({})", stringify!($iter_name), stringify!($field_name), self.$field_name);
                                }
                            )+
                            for i in 0..self.$field_name {
                                $(
                                    bytes.extend(self.$iter_name[i as usize].pack_to_bytes());
                                )+
                            }

                    )?
                )*
                bytes
            }
        }
    };
}

macro_rules! impl_unpack_from_stream {
    (
        $struct_name:ident {
            $(
                $field_name:ident: $field_type:ty $({
                    $($iter_name:ident: $iter_type:ty,)+
                })?
            ,)*
        }
    ) => {
        // Sometimes messages may be empty (length/code only) and so stream is "unused"
        #[allow(unused_variables)]
        impl UnpackFromBytes for $struct_name {
            fn unpack_from_bytes(stream: &mut Vec<u8>) -> Self {
                $(
                    let $field_name = <$field_type>::unpack_from_bytes(stream);
                    $(
                            $(
                                let mut $iter_name: Vec<$iter_type> = Vec::with_capacity($field_name as usize);
                            )+
                            for _ in (0 as $field_type)..$field_name {
                                $(
                                    $iter_name.push(<$iter_type>::unpack_from_bytes(stream));
                                )+
                            }
                    )?
                )*

                $struct_name {
                    $(
                        $field_name,
                        $($($iter_name,)+)?
                    )*
                }
            }
        }
    };
}

/// Creates an empty struct and implements MessageTrait for it
macro_rules! impl_message_trait {
    (
        $struct_name:ident<$to_send:ty, $to_receive:ty> $message_code:tt
    ) => {
        impl MessageTrait for $struct_name {
            type ToSend = $to_send;
            type ToReceive = $to_receive;
            const CODE: MessageType = $message_code;
        }
    };
}

/// Defines a struct and implements PackToBytes for it
macro_rules! define_message_to_send {
    ($($tokens:tt)+) => {
        generate_struct!($($tokens)+);
        impl_pack_to_bytes!($($tokens)+);
    };
}

/// Defines a struct and implements UnpackFromBytes for it
macro_rules! define_message_to_receive {
    ($($tokens:tt)+) => {
        generate_struct!($($tokens)+);
        impl_unpack_from_stream!($($tokens)+);
    };
}

/// Defines a struct and implements PackToBytes and UnpackFromBytes for it
macro_rules! define_message_to_send_and_receive {
    ($($tokens:tt)+) => {
        generate_struct!($($tokens)+);
        impl_pack_to_bytes!($($tokens)+);
        impl_unpack_from_stream!($($tokens)+);
    };
}
