//! Exact acquisition operations for the actual canonical nominal constructor.
//! Legacy identity-only construction retains its independent unpriced helpers.
use super::*;
use std::mem::size_of;
mod types;
pub(super) enum Payer<'a, E> {
    Legacy(&'a mut dyn FnMut(&str) -> Result<DeclarationId, E>),
    Funded(&'a mut dyn FnMut(usize) -> Result<(), E>),
}
impl<E> Payer<'_, E> {
    pub(super) fn bytes(&mut self, bytes: usize) -> Result<(), E> {
        match self { Self::Legacy(_) => Ok(()), Self::Funded(reserve) => reserve(bytes) }
    }
    pub(super) fn identity(&mut self, text: &str) -> Result<DeclarationId, E> {
        match self { Self::Legacy(identity) => identity(text), Self::Funded(reserve) => DeclarationId::paid_borrowed(text, reserve) }
    }
    pub(super) fn text(&mut self, text: &str) -> Result<String, E> {
        self.bytes(size_of::<String>().saturating_add(text.len()))?;
        Ok(text.to_owned())
    }
    pub(super) fn node<K,V>(&mut self) -> Result<(), E> {
        self.bytes(crate::cleanup_plan::replay_btree_node_upper::<K,V>().expect("fixed constructor map layout"))
    }
    pub(super) fn vec<T>(&mut self, len: usize) -> Result<Vec<T>, E> {
        self.bytes(size_of::<Vec<T>>().saturating_add(len.saturating_mul(size_of::<T>())))?;
        let values=Vec::with_capacity(len); assert_eq!(values.capacity(),len); Ok(values)
    }
    pub(super) fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), E> {
        if values.capacity()-values.len() >= additional { return Ok(()); }
        let capacity=values.capacity().saturating_mul(2).max(values.len().saturating_add(additional)).max(4);
        let mut next=self.vec(capacity)?;
        next.append(values); *values=next; Ok(())
    }
    pub(super) fn top(&mut self, name: &str, kind: DeclarationKind) -> Result<(), E> {
        // Caller creates one name; insert_top_level clones the map key once.
        self.bytes(size_of::<String>().saturating_add(name.len()))?;
        if matches!(kind, DeclarationKind::Resource|DeclarationKind::Record|DeclarationKind::Class|DeclarationKind::Variant|DeclarationKind::Function) {
            self.node::<String,DeclarationId>()?;
        }
        self.node::<DeclarationId,Declaration>()?;
        self.bytes(2*size_of::<Declaration>())
    }
    pub(super) fn owned(&mut self) -> Result<(), E> {
        self.node::<DeclarationId,Declaration>()?;
        self.bytes(2*size_of::<Declaration>())
    }
    pub(super) fn field(&mut self, name: &str) -> Result<(), E> {
        // One metadata name and its owner/name key; the original field/type
        // remains in the guarded canonical sequence and is never deep-cloned.
        self.bytes(2*size_of::<String>()+2*name.len())?;
        self.node::<(DeclarationId,String),DeclarationId>()?;
        self.owned()
    }
    pub(super) fn case(&mut self, name: &str) -> Result<(), E> {
        self.bytes(size_of::<String>()+name.len())?;
        self.node::<(DeclarationId,String),DeclarationId>()?;
        self.owned()
    }
    pub(super) fn resolve_type(&mut self,index:&DeclarationIndex,ty:&Type,owner:Option<&DeclarationId>) -> Result<Option<ResolvedType>,E> {
        if matches!(self,Self::Legacy(_)) { return Ok(index.resolve_source_type(ty,owner)); }
        types::resolve(index,ty,owner,self)
    }
}
impl DeclarationIndex {
    pub(super) fn insert_field_view(&mut self, owner:DeclarationId, field:&ResolvedFieldDeclaration, origin:IdentityOrigin) {
        self.fields_by_owner_name.insert((owner.clone(),field.name.clone()),field.id.clone());
        self.declarations.insert(field.id.clone(),Declaration { id:field.id.clone(), name:field.name.clone(), kind:DeclarationKind::Field, identity_origin:origin, owner:Some(owner) });
    }
    pub(super) fn insert_case_field_view(&mut self, owner:DeclarationId, field:&ResolvedFieldDeclaration, origin:IdentityOrigin) {
        self.fields_by_owner_name.insert((owner.clone(),field.name.clone()),field.id.clone());
        self.declarations.insert(field.id.clone(),Declaration { id:field.id.clone(), name:field.name.clone(), kind:DeclarationKind::CaseField, identity_origin:origin, owner:Some(owner) });
    }
}
