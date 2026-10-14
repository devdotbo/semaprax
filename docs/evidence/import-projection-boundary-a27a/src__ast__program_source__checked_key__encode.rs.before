//! Exactly the historical Program wire; an incomplete owner cannot encode.
use super::*;
use crate::cache_codec::{Codec, Decoder, Encoder, Result};
fn sequence<'a, T: Codec + 'a>(
    out: &mut Encoder,
    values: impl ExactSizeIterator<Item = &'a T>,
) -> Result<()> {
    out.nested(|out| {
        out.length(values.len())?;
        for value in values {
            value.encode(out)?;
        }
        Ok(())
    })
}
impl Codec for CheckedSource {
    fn encode(&self, out: &mut Encoder) -> Result<()> {
        if let Self::Owned(program) = self {
            return program.encode(out);
        }
        let ready = self
            .ready()
            .ok_or_else(|| crate::cache_codec::grammar("checked source transfer is incomplete"))?;
        out.nested(|out| {
            out.text(ready.path())?;
            out.text(ready.module())?;
            sequence(out, ready.module_uses().iter())?;
            sequence(out, ready.permits().iter())?;
            sequence(out, ready.types().iter())?;
            sequence(out, ready.interfaces().iter())?;
            sequence(out, ready.protocols().iter())?;
            sequence(out, ready.implementations().iter())?;
            sequence(out, ready.agents().iter())?;
            sequence(out, ready.functions().iter())?;
            sequence(out, ready.session_protocols().iter())
        })
    }
    fn decode(input: &mut Decoder<'_>) -> Result<Self> {
        Program::decode(input).map(Self::Owned)
    }
}
