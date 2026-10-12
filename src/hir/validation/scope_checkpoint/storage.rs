//! Funded continuation/checkpoint directories, retaining replacement debits.
use super::*;
#[derive(Clone,Copy)]
pub(super) enum Request {
    Ordinary,
    #[cfg(test)] AllocationFailure,
    #[cfg(test)] ExcessCapacity,
}
pub(in crate::hir::validation) struct Storage<T> {values: Vec<T>}
impl<T> Storage<T> {
    pub(in crate::hir::validation) fn new() -> Self {Self{values:Vec::new()}}
    fn reserve(values: &mut Vec<T>,count: usize,request: Request)->Result<(),Diagnostic> {
        let bytes=count.checked_mul(std::mem::size_of::<T>());
        charge::reserve(bytes)?;
        let bytes=bytes.expect("checked continuation request");
        #[cfg(test)]
        if matches!(request,Request::AllocationFailure) {
            return Err(charge::failure(bytes,"scope continuation directory allocation failed"));
        }
        let actual=match request {
            Request::Ordinary=>count,
            #[cfg(test)] Request::ExcessCapacity=>count.checked_add(1).ok_or_else(||charge::failure(usize::MAX,"injected capacity layout overflowed"))?,
            #[cfg(test)] Request::AllocationFailure=>unreachable!(),
        };
        values.try_reserve_exact(actual-values.len()).map_err(|_|charge::failure(bytes,"scope continuation directory allocation failed"))?;
        if values.capacity()!=count {
            // Unexpected actual backing receives its debit before any element
            // is copied or published. Keep both the original request and the
            // excess debit; a physical refusal has no refund or retry credit.
            if values.capacity()>count {
                charge::reserve((values.capacity()-count).checked_mul(std::mem::size_of::<T>()))?;
            }
            return Err(charge::failure(values.capacity().saturating_mul(std::mem::size_of::<T>()),"scope continuation directory capacity changed"));
        }
        Ok(())
    }
    pub(in crate::hir::validation) fn with_capacity(count: usize) -> Result<Self,Diagnostic> {
        let mut values=Vec::new();Self::reserve(&mut values,count,Request::Ordinary)?;Ok(Self{values})
    }
    #[cfg(test)]
    pub(super) fn with_frontier(count: usize,request: Request)->Result<Self,Diagnostic> {
        let mut values=Vec::new();Self::reserve(&mut values,count,request)?;Ok(Self{values})
    }
    pub(in crate::hir::validation) fn push(&mut self,value: T) -> Result<(),Diagnostic> {
        if self.values.len()==self.values.capacity() {
            let count=self.values.capacity().checked_mul(2).and_then(|n|n.checked_add(1))
                .ok_or_else(||charge::failure(usize::MAX,"scope continuation directory layout overflowed"))?;
            Self::reserve(&mut self.values,count,Request::Ordinary)?;
        }
        self.values.push(value);Ok(())
    }
    pub(in crate::hir::validation) fn pop(&mut self) -> Option<T> {self.values.pop()}
    pub(in crate::hir::validation) fn len(&self) -> usize {self.values.len()}
    #[cfg(test)]
    pub(in crate::hir::validation) fn iter(&self) -> std::slice::Iter<'_,T> {self.values.iter()}
    #[cfg(test)]
    pub(in crate::hir::validation) fn capacity(&self) -> usize {self.values.capacity()}
    pub(in crate::hir::validation) fn split_first(&self) -> Option<(&T,&[T])> {self.values.split_first()}
}
impl<T> std::ops::Index<usize> for Storage<T> {type Output=T;fn index(&self,index:usize)->&T {&self.values[index]}}
impl<T> std::ops::IndexMut<usize> for Storage<T> {fn index_mut(&mut self,index:usize)->&mut T {&mut self.values[index]}}
