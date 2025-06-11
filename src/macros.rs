macro_rules! generate_struct {
    (
        $struct_name:ident {
            $(
                $(($_:ident))? $field_name:ident: $field_type:ty $({
                    $($iter_name:ident: $iter_type:ty,)+
                })?
            ,)*
        }
    ) => {
        // Debug isn't always used
        #[derive(Debug, Clone)]
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
                $(($_:ident))? $field_name:ident: $field_type:ty $(
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
                let mut bytes: Vec<u8> = Vec::new();
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

macro_rules! unpack {
    (
        $optional:ident $field_name:ident: $field_type:ty => $stream:ident
    ) => {
        let $field_name = <$field_type>::unpack_from_bytes($stream).unwrap_or(None);
    };
    (
        $field_name:ident: $field_type:ty => $stream:ident
    ) => {
        let $field_name = <$field_type>::unpack_from_bytes($stream)?;
    };
}

macro_rules! impl_unpack_from_stream {
    (
        $struct_name:ident {
            $(
                $(($optional:ident))? $field_name:ident: $field_type:ty
            ,)*
        }
    ) => {
        // Sometimes messages may be empty (length/code only) and so stream is "unused"
        #[allow(unused_variables)]
        impl UnpackFromBytes for $struct_name {
            fn unpack_from_bytes(stream: &mut Vec<u8>) -> Option<Self> {
                $(
                    unpack!($($optional)? $field_name: $field_type => stream);
                )*

                Some(
                    $struct_name {
                        $(
                            $field_name,
                        )*
                    }
                )
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

/// Writes code to render widgets and focus them when necessary, to be used when implementing `Widget` for `Window`
///
/// Usage is as follows ()
///
/// ```
/// SELF: self,
/// BUFFER: buffer_area,
/// 0 = (self.attr_widget) => render_area,
/// 1 = (local_widget) => render_area2,
/// ````
///
/// This assumes the following:
///
/// - `self` is the self (this has to be passed so the macro can access required attributes/methods)
/// - `buffer_area` is of type `&mut ratatui::prelude::Buffer`
/// - `self.attr_widget` is the widget to be rendered
/// - `self.attr_widget` gains focus at focus index `0`
/// - `self.attr_widget` will be rendered on `render_area`
///
/// (and so on):
macro_rules! render_widgets {
    (
        SELF: $self:ident,
        BUFFER: $buf:ident,
        $($num:literal = ($($widget:tt)+) $([$($state:tt)*])? $(($($focus_func:tt)*))? => $area:expr,)+
    ) => {
        $(
            $($widget)+.clone().render($area, $buf);
        )+

        match $self.get_focused_index() {
            $(
                $num => {
                    make_focused!(
                        ($($widget)+)
                        $($($focus_func)*)?
                    );
                    $($widget)+.render($area, $buf);
                }
            )+
            _ => unimplemented!()
        }
    };
}

macro_rules! make_focused {
    (
        ($($widget:tt)+)
    ) => {
        $($widget)+.make_focused();
    };
    (
        ($($widget:tt)+) $($focus_func:tt)*
    ) => {
        $($focus_func)*;
    };
}

/// Creates the `WindowEnum` enum (doing it manually involves lots of boilerplate).
/// Also implements a mutable getter for each WindowType in `App.windows`
/// (assuming `windows` contains one of each `WindowType` at fixed locations)
///
/// **Usage (repeatable):**
/// ```
/// WindowType get_mut_func 0,
/// ```
/// This assumes `App.windows` contains `WindowEnum::WindowType(_)` at index 0.
macro_rules! make_window_enum {
    (
        ($($all_lifetimes:tt)+),
        $(
            $type:ident $get_mut_func:ident $num:literal ($($lifetime:tt)+),
        )+
    ) => {
        #[derive(Clone)]
        enum WindowEnum<$($all_lifetimes)+> {
            $(
                $type($type<$($lifetime)+>),
            )+
        }

        #[allow(dead_code)]
        impl<$($all_lifetimes)+> WindowEnum<$($all_lifetimes)+> {
            fn get_title(&self) -> String {
                match self {
                    $(
                        WindowEnum::$type(window) => window.get_title(),
                    )+
                }
            }

            fn get_hints(&self) -> Vec<(Event, String)> {
                match self {
                    $(
                        WindowEnum::$type(window) => window.get_hints(),
                    )+
                }
            }

            fn number_of_widgets(&self) -> u8 {
                match self {
                    $(
                        WindowEnum::$type(window) => window.number_of_widgets(),
                    )+
                }
            }

            fn get_focused_index(&self) -> u8 {
                match self {
                    $(
                        WindowEnum::$type(window) => window.get_focused_index(),
                    )+
                }
            }

            fn set_focused_index(&mut self, index: u8) {
                match self {
                    $(
                        WindowEnum::$type(window) => window.set_focused_index(index),
                    )+
                }
            }

            fn perform_action(&mut self, focus_index: u8, event: Event, write_queue: &Sender<SLSKEvents>) {
                match self {
                    $(
                        WindowEnum::$type(window) => window.perform_action(focus_index, event, &write_queue),
                    )+
                }
            }
        }

        impl<$($all_lifetimes)+> App<$($all_lifetimes)+> {
            $(
                fn $get_mut_func<'get>(&'get mut self) -> &'get mut $type<$($lifetime)+> {
                    match self.windows[$num] {
                        WindowEnum::$type(ref mut window) => window,
                        _ => unimplemented!()
                    }
                }
            )+
        }
    }
}
