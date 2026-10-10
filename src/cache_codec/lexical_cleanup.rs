//! Selective append-only cleanup cache projection; legacy bytes stay exact.
use super::{codec_struct, Codec, Decoder, Encoder, Result};
use crate::cleanup_plan::{
    CleanupPlan, LexicalControlPlan, LexicalExitCertificate, LoopCertificate, ResumeCertificate,
    CLEANUP_PLAN_SCHEMA_V18,
};

codec_struct!(LoopCertificate { target, condition, body, latch, region, condition_entry,
    latch_entry, iteration_end, after, entry_order });
codec_struct!(LexicalExitCertificate { site, target, kind, exit, resume });
codec_struct!(ResumeCertificate { entry, blocks });
codec_struct!(LexicalControlPlan { schema, loops, exits, resumes, abrupt_only_regions,
    function_success_exits });

impl Codec for CleanupPlan {
    fn encode(&self, out: &mut Encoder) -> Result<()> {
        if (self.schema == CLEANUP_PLAN_SCHEMA_V18) != self.lexical_control.is_some() {
            return Err(super::grammar("cleanup lexical certificate/schema mismatch"));
        }
        out.nested(|out| {
            self.schema.encode(out)?;
            self.entry.encode(out)?;
            self.entry_state.encode(out)?;
            self.slots.encode(out)?;
            self.status_sources.encode(out)?;
            self.blocks.encode(out)?;
            self.edges.encode(out)?;
            self.regions.encode(out)?;
            self.exits.encode(out)?;
            if let Some(control) = &self.lexical_control {
                control.encode(out)?;
            }
            Ok(())
        })
    }

    fn decode(input: &mut Decoder<'_>) -> Result<Self> {
        input.nested(|input| {
            let schema = Codec::decode(input)?;
            let entry = Codec::decode(input)?;
            let entry_state = Codec::decode(input)?;
            let slots = Codec::decode(input)?;
            let status_sources = Codec::decode(input)?;
            let blocks = Codec::decode(input)?;
            let edges = Codec::decode(input)?;
            let regions = Codec::decode(input)?;
            let exits = Codec::decode(input)?;
            let lexical_control = if schema == CLEANUP_PLAN_SCHEMA_V18 {
                Some(Codec::decode(input)?)
            } else {
                None
            };
            Ok(Self { schema, entry, entry_state, slots, status_sources, blocks,
                edges, regions, exits, lexical_control })
        })
    }
}
