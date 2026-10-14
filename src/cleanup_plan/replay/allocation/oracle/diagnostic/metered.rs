//! Opted live Oracle diagnostic receiving carrier. A single paid pass removes
//! the original unmetered counting pass. Every actual write and replacement
//! backing is charged on the same finite account before the operation; old
//! diagnostic receivers remain complete and independent in the parent.
use super::*;

struct Text {
    lease: OracleLease,
    bytes: String,
    first: Option<ReplayFailure>,
}
impl Text {
    fn append(&mut self, value: &str) -> Result<(), ReplayFailure> {
        self.lease.charge_work(1).map_err(ReplayFailure::Account)?;
        self.lease
            .charge_work(value.len())
            .map_err(ReplayFailure::Account)?;
        let required =
            self.bytes.len().checked_add(value.len()).ok_or_else(|| {
                physical(self.lease, "oracle diagnostic string length overflowed")
            })?;
        if required > self.bytes.capacity() {
            let doubled = self.bytes.capacity().checked_mul(2).ok_or_else(|| {
                physical(self.lease, "oracle diagnostic string capacity overflowed")
            })?;
            let target = required.max(doubled).max(8);
            self.lease
                .charge_work(self.bytes.len())
                .map_err(ReplayFailure::Account)?;
            if self.bytes.capacity() != 0 {
                self.lease.charge_work(1).map_err(ReplayFailure::Account)?;
            }
            self.lease.charge(target).map_err(ReplayFailure::Account)?;
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| physical(self.lease, "oracle diagnostic allocation failed"))?;
            if self.bytes.capacity() != target {
                return Err(physical(
                    self.lease,
                    "oracle diagnostic capacity differs from its reservation",
                ));
            }
        }
        self.bytes.push_str(value);
        Ok(())
    }
}
impl Write for Text {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.first.is_some() {
            return Err(fmt::Error);
        }
        match self.append(value) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.first = Some(error);
                Err(fmt::Error)
            }
        }
    }
}
fn receive(
    lease: OracleLease,
    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
    prefix: bool,
) -> Result<Diagnostic, ReplayFailure> {
    if !matches!(super::super::super::authority_for_function(function), Ok(Some(current)) if current == lease)
    {
        return Err(physical(
            lease,
            "oracle diagnostic invocation differs from the live function",
        ));
    }
    // Formatting dispatch and final carrier/buffer release are bounded scalar
    // operations. Dynamic UTF-8 writes, retained copies and backing are paid
    // separately below. This is work, not a conversion from capacity bytes.
    lease.charge_work(3).map_err(ReplayFailure::Account)?;
    let mut text = Text {
        lease,
        bytes: String::new(),
        first: None,
    };
    let formatted = if prefix {
        write!(
            text,
            "cleanup plan for function `{}` failed independent replay: {}",
            function.id, message
        )
    } else {
        write!(text, "{message}")
    };
    if let Some(error) = text.first {
        return Err(error);
    }
    if formatted.is_err() {
        return Err(physical(lease, "oracle diagnostic formatter failed"));
    }
    Ok(Diagnostic::io("SPX-H006", text.bytes))
}
pub(super) fn replay(
    lease: OracleLease,
    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    receive(lease, function, message, true)
}
pub(in crate::cleanup_plan::replay) fn raw(
    lease: OracleLease,
    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    receive(lease, function, message, false)
}
