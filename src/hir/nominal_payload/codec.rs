//! Exact old Vec/map wire; private allocation representation is not encoded.
use super::*;
use crate::cache_codec::{Codec, Decoder, Encoder};
impl<T: NominalElement + Codec> Codec for NominalPayload<T> {
    fn encode(&self, e: &mut Encoder) -> crate::cache_codec::Result<()> {
        self.deref().encode(e)
    }
    fn decode(d: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        Vec::<T>::decode(d).map(Into::into)
    }
}
impl<V: Codec> Codec for map::IndexMap<V> {
    fn encode(&self, e: &mut Encoder) -> crate::cache_codec::Result<()> {
        self.0.encode(e)
    }
    fn decode(d: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        std::collections::BTreeMap::decode(d).map(Self)
    }
}
impl Codec for map::CaseFields {
    fn encode(&self, e: &mut Encoder) -> crate::cache_codec::Result<()> {
        self.deref().encode(e)
    }
    fn decode(d: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        Vec::<ResolvedFieldDeclaration>::decode(d).map(Into::into)
    }
}
