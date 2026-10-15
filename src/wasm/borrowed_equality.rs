//! Additive helper for the existing public borrowed-text carrier.
use super::{
    write_i64, write_u32, BinaryOp, ByteOutput, Diagnostic, ResolvedExprKind, ResolvedProgram,
    ResolvedType,
};
use crate::hir::lexical_exit::ChildCursor;

pub(super) fn uses(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    const DEPTH: usize = crate::cache_codec::MAX_DEPTH;
    if !crate::bounded_output::reserve_active(std::mem::size_of::<
        [Option<ChildCursor<'_>>; DEPTH + 1],
    >()) {
        return Err(Diagnostic::io(
            "SPX-W119",
            "Str equality census backing refused",
        ));
    }
    let mut frames: [Option<ChildCursor<'_>>; DEPTH + 1] = std::array::from_fn(|_| None);
    let mut work = crate::cache_codec::MAX_NODES;
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        for root in std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures)
        {
            frames[0] = Some(ChildCursor::new(root));
            let mut depth = 1;
            while depth != 0 {
                work = work.checked_sub(1).ok_or_else(|| {
                    Diagnostic::io("SPX-W119", "Str equality census work refused")
                })?;
                let cursor = frames[depth - 1].as_mut().expect("held cursor");
                if !cursor.is_started()
                    && matches!(&cursor.expression().kind,
                    ResolvedExprKind::Binary { op: BinaryOp::Eq | BinaryOp::Ne, left, .. } if left.ty == ResolvedType::Str)
                {
                    return Ok(true);
                }
                if let Some(child) = cursor.next() {
                    let next = frames.get_mut(depth).ok_or_else(|| {
                        Diagnostic::io("SPX-W119", "Str equality census depth refused")
                    })?;
                    *next = Some(ChildCursor::new(child));
                    depth += 1;
                } else {
                    frames[depth - 1] = None;
                    depth -= 1;
                }
            }
        }
    }
    Ok(false)
}

pub(super) fn emit_body(body: &mut impl ByteOutput, starts_with: u32) {
    write_u32(body, 0); // no locals: arguments have already been staged once
    body.extend_bytes(&[0x20, 0x00, 0x20, 0x01, 0x10]);
    write_u32(body, starts_with);
    for parameter in [0, 1] {
        body.extend_bytes(&[0x20, parameter, 0x42]);
        write_i64(body, 32);
        body.push(0x88); // pointer-low/length-high public text carrier
    }
    body.extend_bytes(&[0x51, 0x71, 0x0b]); // exact length equality AND prefix
}
