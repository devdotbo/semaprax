# Actual transient module registration repair

Base bbd8929c86185691269f8c24050dbfbaf807d38f. The owning compiler emitted E0583 for the new implicit `mod transient;`, looking under tests/transient.rs. The authored child is tests/full/transient.rs. Its registration now uses explicit `#[path = "full/transient.rs"]`, matching the adjacent receiving child. Both children remain byte-identical; all funding and full receiver assertions are preserved. Existing real workspace source inventory already joins both children.

No Cargo/build/tests ran. This is a private next-batch patch; the frozen279 manifest and artifacts remain unchanged. Root must rebind in a fresh round after all compile repairs are integrated.
