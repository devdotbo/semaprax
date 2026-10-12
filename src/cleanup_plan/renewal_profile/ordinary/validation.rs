//! Demand one ordinary family at its original current-member consumption frontier.
use super::*;
impl<'a> Directory<'a> {
    pub(crate) fn collect_validation_family(
        member: CurrentMember<'a>,
        family: Family,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.units(1)?;
        Self::build_inner(
            member.program,
            member.function,
            admission,
            false,
            |output, admission| match family {
                Family::Iterator => {
                    iterator::collect(member.program, member.function, output, admission)
                }
                Family::Vec => {
                    vector::collect(member.program, member.function, output, admission)?;
                    consuming::collect(member.program, member.function, output, admission)
                }
                Family::ByteSet | Family::String => {
                    leaf::collect(member.function, family, output, admission)
                }
                Family::Helper => admission.refuse(Refusal::Subject),
            },
        )
    }
}
