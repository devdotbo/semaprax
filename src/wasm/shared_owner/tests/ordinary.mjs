// Runs one ordinary emitted module against the private lease-aware host and
// requires the unchanged ordinary result (or selected status) with complete
// settlement. `lease` makes every nonempty allocation a registry lease.
import fs from "node:fs";
import { createPrivateOrdinaryByteHost } from "./host.mjs";

const [modulePath, expectedText, mode = "plain"] = process.argv.slice(2);
if (mode !== "plain" && mode !== "lease") throw new Error(`unknown mode ${mode}`);
const expectedStatus = expectedText.startsWith("status:") ? Number(expectedText.slice(7)) : 0;
const expected = expectedStatus === 0 ? BigInt(expectedText) : null;
const moduleBytes = fs.readFileSync(modulePath);
const host = createPrivateOrdinaryByteHost({ leaseWitness: mode === "lease" });
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
let leased = 0;
for (let run = 0; run < 4; run += 1) {
  const receipt = host.facade.beginInvocation();
  let selected = 0, value;
  try {
    value = instance.exports.semaprax_main();
  } catch (error) {
    if (!String(error.message).startsWith("status:")) throw error;
    selected = Number(error.message.slice(7));
  }
  const stats = host.facade.stats();
  if (selected !== expectedStatus || (selected === 0 && value !== expected)) {
    throw new Error(`ordinary result ${selected}/${value} ${JSON.stringify(stats)}`);
  }
  // Every owner, lease, backing and authority settled; the explicit end
  // receipt below refuses otherwise.
  if (stats.entries !== 0 || stats.vectors !== 0 || stats.iterators !== 0 || stats.boxes !== 0
      || stats.liveLeases !== 0 || stats.liveBackings !== 0) {
    throw new Error(`ordinary settlement ${JSON.stringify(stats)}`);
  }
  host.facade.audit();
  host.facade.endInvocation(receipt);
  leased = stats.releasedBackings;
}
if (mode === "lease" && leased === 0) throw new Error("lease witness issued no lease");
console.log("shared-owner-ordinary-ok");
