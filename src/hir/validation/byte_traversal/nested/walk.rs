//! One preowned iterative cursor stack, reused by census passes and roots.
//! Closures lend evaluated captures only, exactly as the original census.
use super::{Failure, Storage};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFunction};
use std::mem::size_of;

#[derive(Clone, Copy)]
struct Frame<'a> {
    cursor: crate::hir::lexical_exit::ChildCursor<'a>,
    capture: usize,
    entered: bool,
    position: usize,
}
#[derive(Clone, Copy)]
pub(super) enum Event<'a> {
    Enter {
        expression: &'a ResolvedExpr,
        position: usize,
    },
    Exit {
        expression: &'a ResolvedExpr,
        position: usize,
        end: usize,
    },
}
pub(super) struct Walker<'a> {
    frames: [Option<Frame<'a>>; crate::cache_codec::MAX_DEPTH + 1],
}
impl<'a> Walker<'a> {
    pub(super) fn acquire(storage: &Storage) -> Result<Self, Failure> {
        storage.reserve(size_of::<Self>() + size_of::<Result<Self, Failure>>())?;
        Ok(Self {
            frames: [None; crate::cache_codec::MAX_DEPTH + 1],
        })
    }
    pub(super) fn function(
        &mut self,
        function: &'a ResolvedFunction,
        mut visit: impl FnMut(Event<'a>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let mut ordinal = 0usize;
        for root in function
            .ensures
            .iter()
            .rev()
            .chain(std::iter::once(&function.body))
            .chain(function.requires.iter().rev())
        {
            self.root(root, &mut ordinal, &mut visit)?;
        }
        Ok(())
    }
    fn root(
        &mut self,
        root: &'a ResolvedExpr,
        ordinal: &mut usize,
        visit: &mut impl FnMut(Event<'a>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        // A prior failed/finished pass cannot leave authorization or a cursor
        // in the next root. Clearing only borrowed Copy rows cannot allocate.
        self.frames.fill(None);
        let frame = |expression| Frame {
            cursor: crate::hir::lexical_exit::ChildCursor::new(expression),
            capture: 0,
            entered: false,
            position: 0,
        };
        self.frames[0] = Some(frame(root));
        let mut len = 1;
        while len != 0 {
            let current = self.frames[len - 1]
                .as_mut()
                .expect("occupied census frame");
            let expression = current.cursor.expression();
            if !current.entered {
                current.position = *ordinal;
                *ordinal = ordinal.checked_add(1).ok_or_else(|| {
                    Failure::semantic("borrowed byte traversal census identity count overflows")
                })?;
                visit(Event::Enter {
                    expression,
                    position: current.position,
                })?;
                current.entered = true;
            }
            let next = if let ResolvedExprKind::Closure { captures, .. } = &expression.kind {
                let next = captures.get(current.capture).map(|capture| &capture.value);
                current.capture += usize::from(next.is_some());
                next
            } else {
                current.cursor.next()
            };
            if let Some(next) = next {
                if len == self.frames.len() {
                    return Err(Failure::semantic(
                        "borrowed byte traversal scan exceeds admitted expression depth",
                    ));
                }
                self.frames[len] = Some(frame(next));
                len += 1;
            } else {
                visit(Event::Exit {
                    expression,
                    position: current.position,
                    end: *ordinal,
                })?;
                len -= 1;
                self.frames[len] = None;
            }
        }
        Ok(())
    }
}
