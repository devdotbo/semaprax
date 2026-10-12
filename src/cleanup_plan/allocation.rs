//! Closed provenance vocabulary for the additive checked-allocation projection.
//! These data carry no preparation, source, allocator or cleanup authority.
//! Frozen ordinary plans never select this lane/schema.

pub const ALLOCATION_DOMAIN_V1: &str = "semaprax.allocation.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationCase {
    PayloadRefused,
    SizeOverflow,
}
impl AllocationCase {
    pub const fn code(self) -> u32 {
        match self { Self::PayloadRefused => 1, Self::SizeOverflow => 2 }
    }
}

/// Kind of the actual allocating materialization, independently authenticated
/// against the current expression and its instantiated element/field layout.
/// A collection kind includes each partial prefix allocation; it is not a
/// promise that a collection operation performs exactly one allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationOperation {
    StringLiteral,
    StringClone,
    StringConcat,
    StringFromChar,
    StringFromI64,
    StringFromUsize,
    StringFromStr,
    StringFromUtf8,
    StringSlice,
    StringTrim,
    LiteralFormat,
    BytesCopy,
    BytesZeroed,
    VecBacking,
    VecElementClone,
    MapBacking,
    MapKeyClone,
    SetBacking,
    SetKeyClone,
    BoxBacking,
    RecordClone,
    ClosureBacking,
    ClosureCaptureClone,
}
impl AllocationOperation {
    pub const fn name(self) -> &'static str {
        match self {
            Self::StringLiteral => "string_literal",
            Self::StringClone => "string_clone",
            Self::StringConcat => "string_concat",
            Self::StringFromChar => "string_from_char",
            Self::StringFromI64 => "string_from_i64",
            Self::StringFromUsize => "string_from_usize",
            Self::StringFromStr => "string_from_str",
            Self::StringFromUtf8 => "string_from_utf8",
            Self::StringSlice => "string_slice",
            Self::StringTrim => "string_trim",
            Self::LiteralFormat => "literal_format",
            Self::BytesCopy => "bytes_copy",
            Self::BytesZeroed => "bytes_zeroed",
            Self::VecBacking => "vec_backing",
            Self::VecElementClone => "vec_element_clone",
            Self::MapBacking => "map_backing",
            Self::MapKeyClone => "map_key_clone",
            Self::SetBacking => "set_backing",
            Self::SetKeyClone => "set_key_clone",
            Self::BoxBacking => "box_backing",
            Self::RecordClone => "record_clone",
            Self::ClosureBacking => "closure_backing",
            Self::ClosureCaptureClone => "closure_capture_clone",
        }
    }
}
