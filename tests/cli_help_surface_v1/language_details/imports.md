### Record import visibility details

- Import every type you name from a function signature, including nested
  types and factory results. A private inferred signature may retain hidden
  layout facts; it does not let you name that type, its cases or siblings in
  annotations, patterns or constructors. Use an explicit `use type` for those.
  With `std.pattern.make`, add
  `use type @id("std.pattern.matcher") from std.pattern as Matcher;`
  `SPX-G172` names the missing type identity.
