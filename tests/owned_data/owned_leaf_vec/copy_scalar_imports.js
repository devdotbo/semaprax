// Strict private legacy Copy-scalar imports. Copy-record carriers are packed
// into these scalar words by the compiler; no owned payload tag is admitted.
module.exports = (env, vectorValue) => {
  const scalarTag = tag => {
    if (!Number.isInteger(tag) || tag < 1 || tag > 8)
      throw Error('legacy Copy import received an owning or unknown tag');
  };
  for (const name of ['with_capacity', 'push', 'len', 'capacity', 'get', 'reserve_exact', 'set', 'clear']) {
    env[`spx_vec_${name}`] = (...args) => {
      scalarTag(args[name === 'with_capacity' ? 0 : 1]);
      return env[`spx_vec_${name}_v2`](...args);
    };
  }
  env.spx_vec_drop = handle => {
    const value = vectorValue(handle);
    if (!value) throw Error('stale legacy Copy vector drop');
    scalarTag(value.tag);
    return env.spx_vec_drop_v2(handle);
  };
};
