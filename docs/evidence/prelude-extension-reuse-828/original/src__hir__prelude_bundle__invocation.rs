//! One actual core invocation retains one base owner and its first typed failure.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InvocationFailure<E> {
    Bytes(E),
    Work(WorkRefusal),
    Producer(ProducerFailure),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProducerFailure {
    Layout,
    Allocation,
    Capacity,
    Schema,
    Detached,
}
pub(crate) struct Invocation<E> {
    bundle: Option<BasePreludeBundle>,
    first: Option<InvocationFailure<E>>,
    receipt: Option<WorkReceipt>,
    #[cfg(test)]
    work_limit: Option<usize>,
}
impl<E> Invocation<E> {
    pub(crate) fn bind_owner(&mut self, current: bool) {
        if !current && self.first.is_none() {
            self.first = Some(InvocationFailure::Producer(ProducerFailure::Detached));
        }
    }
    pub(crate) fn new() -> Self {
        Self {
            bundle: None,
            first: None,
            receipt: None,
            #[cfg(test)]
            work_limit: None,
        }
    }
}
impl<E: Copy> Invocation<E> {
    pub(crate) fn ensure(
        &mut self,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<&BasePreludeBundle, InvocationFailure<E>> {
        if let Some(first) = self.first {
            return Err(first);
        }
        let result = (|| {
            reserve(0).map_err(InvocationFailure::Bytes)?;
            if self.bundle.is_none() {
                reserve(
                    std::mem::size_of::<Option<BasePreludeBundle>>()
                        + std::mem::size_of::<Result<BasePreludeBundle, Failure<FixedFailure<E>>>>(
                        )
                        + std::mem::size_of::<Result<&BasePreludeBundle, InvocationFailure<E>>>(),
                )
                .map_err(InvocationFailure::Bytes)?;
                let mut payer =
                    FixedSchemaPayer::acquire(reserve).map_err(InvocationFailure::Bytes)?;
                #[cfg(test)]
                if let Some(limit) = self.work_limit {
                    payer.restrict_for_test(limit);
                }
                let result = BasePreludeBundle::acquire(&mut payer);
                self.receipt = Some(payer.receipt());
                self.bundle = Some(result.map_err(|failure| match failure {
                    Failure::Payer(FixedFailure::Bytes(error)) => InvocationFailure::Bytes(error),
                    Failure::Payer(FixedFailure::Work(error)) => InvocationFailure::Work(error),
                    Failure::Layout => InvocationFailure::Producer(ProducerFailure::Layout),
                    Failure::Allocation => InvocationFailure::Producer(ProducerFailure::Allocation),
                    Failure::Capacity => InvocationFailure::Producer(ProducerFailure::Capacity),
                    Failure::Schema => InvocationFailure::Producer(ProducerFailure::Schema),
                })?);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.first = Some(error);
            return Err(error);
        }
        Ok(self
            .bundle
            .as_ref()
            .expect("success retains the complete owner"))
    }
    #[cfg(test)]
    pub(crate) fn restrict_work(&mut self, limit: usize) {
        assert!(self.bundle.is_none() && self.first.is_none());
        self.work_limit = Some(limit);
    }
    #[cfg(test)]
    pub(crate) fn receipt(&self) -> Option<WorkReceipt> {
        self.receipt
    }
}
