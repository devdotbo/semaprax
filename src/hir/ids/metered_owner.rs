//! Exact nonallocating owner probes for the guarded monomorphic helper route.
use super::*;
struct Compare<'a, E, F> {
    actual: &'a [u8],
    charge: &'a mut F,
    error: Option<E>,
}
impl<E, F: FnMut(usize) -> Result<(), E>> fmt::Write for Compare<'_, E, F> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if let Err(error) = (self.charge)(1) {
            self.error = Some(error);
            return Err(fmt::Error);
        }
        if self.actual.len() < value.len() {
            return Err(fmt::Error);
        }
        for expected in value.bytes() {
            if let Err(error) = (self.charge)(1) {
                self.error = Some(error);
                return Err(fmt::Error);
            }
            if self.actual[0] != expected {
                return Err(fmt::Error);
            }
            self.actual = &self.actual[1..];
        }
        Ok(())
    }
}
fn scoped<E>(
    actual: &str,
    declaration: &DeclarationId,
    kind: &str,
    path: &str,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    let mut output = Compare {
        actual: actual.as_bytes(),
        charge,
        error: None,
    };
    let result = write!(
        output,
        "declaration:{}:{}:{kind}:{}:{path}",
        declaration.as_str().len(),
        declaration,
        path.len()
    );
    if let Some(error) = output.error {
        return Err(error);
    }
    (output.charge)(1)?;
    Ok(result.is_ok() && output.actual.is_empty())
}
pub(crate) fn matches<E>(
    function: &super::super::ResolvedFunction,
    owner: &FunctionExecutionId,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    let FunctionExecutionId::Monomorphic(declaration) = owner else {
        return Ok(false);
    };
    let mut fingerprint = 0xcbf29ce484222325u64;
    for byte in function.result_id.0.bytes() {
        charge(1)?;
        fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    charge(1)?;
    if function.result_id.1 != fingerprint {
        return Ok(false);
    }
    if !scoped(
        function.result_id.as_str(),
        declaration,
        "value:result",
        "",
        charge,
    )? {
        return Ok(false);
    }
    scoped(
        function.body.id.as_str(),
        declaration,
        "expression",
        "body",
        charge,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monomorphic_owner_matches_original_hash_and_encoding_with_exact_work_and_no_allocation() {
        let program = crate::hir::resolve(
            &crate::check(
                "module owners; @id(\"main\") fn main()->i64 {0}",
                "metered-owner.spx",
            )
            .unwrap(),
        )
        .unwrap();
        let original = &program.functions[0];
        let owner = FunctionExecutionId::Monomorphic(original.id.clone());
        struct Encoded {
            text: String,
            work: usize,
        }
        impl fmt::Write for Encoded {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                self.work += 1 + value.len();
                self.text.push_str(value);
                Ok(())
            }
        }
        // Independent encoding/storage occurs outside the observer and uses
        // the original owning encoder as an additional exact spelling oracle.
        let mut result = Encoded {
            text: String::new(),
            work: 0,
        };
        let mut body = Encoded {
            text: String::new(),
            work: 0,
        };
        let result_kind = "value:result";
        let result_path = "";
        let body_kind = "expression";
        let body_path = "body";
        write!(
            result,
            "declaration:{}:{}:{}:{}:{}",
            original.id.as_str().len(),
            original.id,
            result_kind,
            result_path.len(),
            result_path
        )
        .unwrap();
        write!(
            body,
            "declaration:{}:{}:{}:{}:{}",
            original.id.as_str().len(),
            original.id,
            body_kind,
            body_path.len(),
            body_path
        )
        .unwrap();
        assert_eq!(result.text, scoped_identity(&owner, "value:result", ""));
        assert_eq!(body.text, scoped_identity(&owner, "expression", "body"));
        let exact = 2 + original.result_id.as_str().len() + result.work + 1 + body.work + 1;
        let (answer, allocation) = crate::test_allocation_observer::measure(|| {
            let mut remaining = exact;
            let answer = matches(original, &owner, &mut |units| {
                remaining = remaining.checked_sub(units).ok_or(())?;
                Ok(())
            });
            assert_eq!(remaining, 0);
            answer
        });
        assert_eq!(answer, Ok(true));
        assert_eq!(allocation.allocated, 0);
        assert_eq!(allocation.live, 0);
        assert!(!allocation.invalid);
        let mut remaining = exact - 1;
        assert_eq!(
            matches(original, &owner, &mut |units| {
                remaining = remaining.checked_sub(units).ok_or(())?;
                Ok(())
            }),
            Err(())
        );
        for mutant in 0..3 {
            let mut function = original.clone();
            match mutant {
                0 => function.result_id.1 ^= 1,
                1 => function.result_id = ValueId::new("forged.result".into()),
                _ => function.body.id = ExpressionId::from_owned("forged.body".into()),
            }
            let ordinary = function.result_id.matches_result(&owner)
                && function.body.id.matches(&owner, "body");
            let (answer, allocation) = crate::test_allocation_observer::measure(|| {
                matches(&function, &owner, &mut |_| Ok::<_, ()>(()))
            });
            assert_eq!(answer, Ok(ordinary));
            assert!(!ordinary);
            assert_eq!(allocation.allocated, 0);
            assert_eq!(allocation.live, 0);
            assert!(!allocation.invalid);
        }
    }
}
