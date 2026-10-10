//! Emission context initialization; target identity is borrowed from the authenticated invocation.
use super::*;
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn new(
        output: &'a mut O,
        variables: HashMap<ValueId, CBinding>,
        function: &'a ResolvedFunction,
        execution: &'a FunctionExecutionId,
        emission: &'a NativeEmissionContext<'a>,
        bytes_plan: Option<&'a native_bytes::NativeBytesPlan>,
        borrowed_aggregate_bytes: HashMap<(ValueId, Vec<DeclarationId>), String>,
        track_strings: bool,
    ) -> Self {
        Self {
            output,
            program: emission.program,
            resource_abi: emission.resource_abi,
            variables,
            mutable_bindings: BTreeSet::new(),
            resource_text_views: BTreeSet::new(),
            function,
            execution,
            functions: emission.functions,
            record_layouts: emission.record_layouts,
            variant_layouts: emission.variant_layouts,
            return_type: &function.return_type,
            bytes_plan,
            borrowed_aggregate_bytes,
            output_profile: emission.output_profile,
            semantic_metered: false,
            owned_strings: track_strings.then(owned_strings::OwnedStrings::default),
            try_target_enabled: false,
            string_owner_moves: crate::literal_format::moving_string_operands(function),
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
            next_local: 0,
            indent: 1,
        }
    }
}
