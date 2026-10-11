//! Selected physical allocation requests; ordinary replay keeps its exact route.
use super::*;

impl ReplayBudget {
    pub(super) fn with_physical(
        function: &ResolvedFunction,
        bytes: usize,
    ) -> Result<Self, Diagnostic> {
        Self::from_reservation(function, physical_storage::PhysicalReservation::new(bytes))
    }

    pub(super) fn from_reservation(
        function: &ResolvedFunction,
        mut physical: physical_storage::PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        physical
            .reserve_array::<physical_storage::PhysicalReservation>(1)
            .map_err(|_| {
                replay_error(
                    function,
                    "cleanup replay physical reservation exceeds the bound",
                )
            })?;
        physical
            .reserve_array::<Option<(&ResolvedType, usize)>>(514)
            .map_err(|_| {
                replay_error(function, "cleanup replay clone cursor reservation refused")
            })?;
        Ok(Self {
            physical: Some(Box::new(physical)),
            ..Self::new()
        })
    }

    pub(super) fn reserve_bytes(
        &mut self,
        function: &ResolvedFunction,
        bytes: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        let Some(physical) = self.physical.as_deref_mut() else {
            return Ok(());
        };
        physical.reserve(bytes).map_err(|_| {
            replay_error(
                function,
                format!("cleanup replay physical reservation exhausted during {phase}"),
            )
        })
    }

    pub(super) fn reserve_clone<T: ClonePayload>(
        &mut self,
        function: &ResolvedFunction,
        value: &T,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.physical.is_none() {
            return Ok(());
        }
        let bytes = value.clone_payload_bytes().ok_or_else(|| {
            replay_error(
                function,
                "cleanup replay clone payload exceeds the physical bound",
            )
        })?;
        self.reserve_bytes(function, bytes, phase)
    }

    pub(super) fn reserve_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        let Some(physical) = self.physical.as_deref_mut() else {
            return Ok(());
        };
        physical.reserve_array::<T>(count).map_err(|_| {
            replay_error(
                function,
                format!("cleanup replay physical reservation exhausted during {phase}"),
            )
        })
    }

    pub(super) fn reserve_btree_node<K, V>(
        &mut self,
        function: &ResolvedFunction,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.physical.is_none() {
            return Ok(());
        }
        let bytes = physical_storage::btree_node_upper::<K, V>().ok_or_else(|| {
            replay_error(function, "cleanup replay physical tree layout overflowed")
        })?;
        self.reserve_bytes(function, bytes, phase)
    }

    pub(super) fn reserve_deque<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut VecDeque<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.physical.is_none() {
            return Ok(());
        }
        self.reserve_bytes(function, 0, phase)?;
        let required = values.len().checked_add(additional).ok_or_else(|| {
            replay_error(function, "cleanup replay physical queue length overflowed")
        })?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = physical_storage::replay_growth_capacity(values.capacity(), required)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup replay physical queue capacity overflowed",
                )
            })?;
        self.reserve_array::<T>(function, target, phase)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| {
                replay_error(function, "cleanup replay physical queue allocation failed")
            })?;
        if values.capacity() != target {
            return Err(replay_error(
                function,
                "cleanup replay physical queue capacity disagrees with its reservation",
            ));
        }
        Ok(())
    }

    /// The old buffer remains fully charged while its replacement is allocated.
    /// Selected growth is geometric; ordinary replay retains its original allocator
    /// growth and performs no extra scans, reservations or allocator calls here.
    pub(super) fn reserve_vec<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut Vec<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.physical.is_none() {
            return Ok(());
        }
        self.reserve_bytes(function, 0, phase)?;
        let required = values.len().checked_add(additional).ok_or_else(|| {
            replay_error(function, "cleanup replay physical vector length overflowed")
        })?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = physical_storage::replay_growth_capacity(values.capacity(), required)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup replay physical vector capacity overflowed",
                )
            })?;
        self.reserve_array::<T>(function, target, phase)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| {
                replay_error(function, "cleanup replay physical vector allocation failed")
            })?;
        if values.capacity() != target {
            return Err(replay_error(
                function,
                "cleanup replay physical vector capacity disagrees with its reservation",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod queue_tests;
