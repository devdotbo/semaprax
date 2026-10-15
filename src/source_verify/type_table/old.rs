//! Complete original type-table constructor and secondary HashMap lookup.
use super::*;
#[rustfmt::skip]
impl<'a> TypeTable<'a> {
    pub(in crate::source_verify) fn new_owned_reference(program: &'a dyn ProgramSource) -> Self {
        let declarations: HashMap<&'a str, &'a TypeDeclaration> = program
            .types()
            .iter()
            .chain(crate::prelude::declarations_for_program(program))
            .map(|declaration| (declaration.name.as_str(), declaration))
            .collect();
        let mut merged_class_fields = HashMap::new();
        let mut declared_fields = HashMap::new();
        for (name, declaration) in &declarations {
            if let TypeDeclarationKind::Record { fields }
            | TypeDeclarationKind::Class { fields, .. } = &declaration.kind
            {
                declared_fields.insert(
                    *name,
                    fields
                        .iter()
                        .map(|field| (field.name.as_str(), field))
                        .collect(),
                );
            }
        }
        for (name, declaration) in &declarations {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let mut chain = Vec::new();
            let mut visited = HashSet::new();
            let mut cursor = *name;
            let rooted = loop {
                let Some(ancestor) = declarations.get(cursor).copied() else {
                    break false;
                };
                if !matches!(ancestor.kind, TypeDeclarationKind::Class { .. })
                    || !visited.insert(cursor)
                {
                    break false;
                }
                chain.push(ancestor);
                let Some(Type::Named {
                    name: parent_name, ..
                }) = ancestor.extends.as_ref()
                else {
                    break true;
                };
                cursor = parent_name.as_str();
            };
            if !rooted {
                continue;
            }
            let mut fields = Vec::new();
            for ancestor in chain.into_iter().rev() {
                let TypeDeclarationKind::Class {
                    fields: declared, ..
                } = &ancestor.kind
                else {
                    unreachable!("class kind checked above")
                };
                fields.extend(declared.iter().cloned());
            }
            merged_class_fields.insert(*name, fields);
        }
        let mut class_parents = HashMap::new();
        for (name, declaration) in &declarations {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            if let Some(Type::Named {
                name: parent_name, ..
            }) = declaration.extends.as_ref()
            {
                class_parents.insert(*name, parent_name.as_str());
            }
        }
        Self {
            native_owned_containers: [
                crate::ast::ImportResult::OwnedOptionString,
                crate::ast::ImportResult::OwnedResultStringI64,
                crate::ast::ImportResult::OwnedResultStringOptionI64,
            ]
            .map(|kind| crate::ast::ImportResult::container_enabled(program, &kind.value_type())),
            declarations,
            declared_fields: Some(declared_fields),
            merged_class_fields,
            class_parents,
            private: None,
        }
    }

    pub(in crate::source_verify) fn declared_field_owned_reference(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&'a FieldDeclaration> {
        self.declared_fields.as_ref().expect("complete old field index")
            .get(type_name)
            .and_then(|fields| fields.get(field_name))
            .copied()
    }

}
