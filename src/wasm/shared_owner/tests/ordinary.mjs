// Runs one ordinary emitted module against the private lease-aware host and
// requires the unchanged ordinary result with complete settlement.
import fs from "node:fs";
import { createPrivateOrdinaryByteHost } from "./host.mjs";

const [modulePath, expectedText] = process.argv.slice(2);
const expected = BigInt(expectedText);
const moduleBytes = fs.readFileSync(modulePath);
const host = createPrivateOrdinaryByteHost();
const env = {
  spx_add: (a, b) => a + b,
  spx_sub: (a, b) => a - b,
  spx_mul: (a, b) => a * b,
  spx_div: (a, b) => a / b,
  spx_rem: (a, b) => a % b,
  spx_neg: a => -a,
  spx_contract_fail: status => { throw new Error(`status:${status}`); },
  ...host.imports,
};
const { instance } = await WebAssembly.instantiate(moduleBytes, { env });
host.bind(instance);
for (let run = 0; run < 4; run += 1) {
  host.facade.beginInvocation();
  const value = instance.exports.semaprax_main();
  const stats = host.facade.stats();
  if (value !== expected || stats.entries !== 0 || stats.vectors !== 0 || stats.iterators !== 0
      || stats.liveLeases !== 0 || stats.liveBackings !== 0 || stats.copies !== stats.drops) {
    throw new Error(`ordinary settlement ${value} ${JSON.stringify(stats)}`);
  }
}
console.log("shared-owner-ordinary-ok");
