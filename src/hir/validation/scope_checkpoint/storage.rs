//! Funded continuation/checkpoint directories, retaining replacement debits.
use super::*;
pub(in crate::hir::validation) struct Storage<T> {values: Vec<T>}
impl<T> Storage<T> {
    pub(in crate::hir::validation) fn new() -> Self {Self{values:Vec::new()}}
    pub(in crate::hir::validation) fn with_capacity(count: usize) -> Result<Self,Diagnostic> {
        charge::reserve(count.checked_mul(std::mem::size_of::<T>()))?;
        let mut values=Vec::new();
        values.try_reserve_exact(count).map_err(|_|hir_error("scope continuation directory allocation failed"))?;
        if values.capacity()!=count {
            return Err(hir_error("scope continuation directory capacity changed"));
        }
        Ok(Self{values})
    }
    pub(in crate::hir::validation) fn push(&mut self,value: T) -> Result<(),Diagnostic> {
        if self.values.len()==self.values.capacity() {
            let count=self.values.capacity().checked_mul(2).and_then(|n|n.checked_add(1));
            let count=count.ok_or_else(||hir_error("scope continuation directory layout overflowed"))?;
            charge::reserve(count.checked_mul(std::mem::size_of::<T>()))?;
            self.values.try_reserve_exact(count-self.values.len()).map_err(|_|hir_error("scope continuation directory allocation failed"))?;
            if self.values.capacity()!=count {
                return Err(hir_error("scope continuation directory capacity changed"));
            }
        }
        self.values.push(value);
        Ok(())
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
