// Test-private namespace. No default host imports, source lowering, public ABI,
// filesystem authority in the lease runtime, or source4096 admission.
import fs from 'node:fs';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const namespace='semaprax.private.fused-row-host.v1';
const names=['start','issue','publish','complete','borrow','read','end_borrow','move_owner','drop_owner','abort','settle_abort','phase'];
const bytes=fs.readFileSync(process.argv[2]);
const digest=process.argv[3];
const count=Number(process.argv[4]),width=Number(process.argv[5]);
assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),digest);
assert(Number.isSafeInteger(count)&&count>0&&count<=4096);
assert(Number.isSafeInteger(width)&&width>0&&count*width<=131072);
const module=await WebAssembly.compile(bytes);
const imports=WebAssembly.Module.imports(module);
assert.deepEqual(imports,names.map(name=>({module:namespace,name,kind:'function'})));
const grants=new WeakSet([module]);
// Issuer is an explicitly held owner, never an ambient/global counter. Its
// nonce cannot reset/wrap; exact object identity is an externref carrier field.
function issuerOwner(){let next=1n;return Object.freeze({mint(){if(next>=0xffffffffffffffffn)throw new Error('issuer-exhaustion');return next++;}});}
const phaseCode={Unstarted:0,Building:1,Complete:2,Aborted:3,Settled:4};
async function instantiate(grant,issuer,metadataLimit=Infinity){
  if(!grants.has(grant))throw new Error('foreign-module');
  // Actual typed-array bytes only. JS engine/closure/BigInt bookkeeping is NOT
  // claimed physically charged; production activation remains closed.
  const required=count*(8+8+1+1+4)+64*(8+4+8+1);
  if(metadataLimit<required)throw new Error('metadata-preflight');
  let phase='Unstarted',issued=0,live=0,pin=false,payload=null,tables=null;
  let allocations=0,releases=0,metadataBytes=0,events=0,cumulative=0;
  function identity(value){if(value!==issuer)throw new Error('foreign-issuer');}
  function phaseIs(value){if(phase!==value)throw new Error('phase');}
  function slot(slot,nonce){
    if(!Number.isInteger(slot)||slot<0||slot>=issued||!tables.live[slot]||tables.nonce[slot]!==BigInt.asUintN(64,nonce))throw new Error('stale-owner');
    return slot;
  }
  function owner(value,index,nonce){identity(value);return slot(index,nonce);}
  function release(){if(live===0&&!pin){if(payload!==null){payload=null;releases++;}phase='Settled';}}
  function allocate(){
    // Complete fixed metadata and one payload acquired only at firstBytes.
    // Host physical OOM propagates, never becomes a recoverable source status.
    tables={nonce:new BigUint64Array(count),generation:new BigUint64Array(count),live:new Uint8Array(count),published:new Uint8Array(count),borrows:new Uint32Array(count),viewNonce:new BigUint64Array(64),viewSlot:new Uint32Array(64),viewGeneration:new BigUint64Array(64),viewLive:new Uint8Array(64)};
    metadataBytes=Object.values(tables).reduce((sum,array)=>sum+array.byteLength,0);
    assert.equal(metadataBytes,required);
    events++;cumulative+=count*width;
    if(events>32||cumulative>2097152)throw new Error('ordinary-byte-capacity');
    payload=new Uint8Array(count*width);allocations++;
  }
  const host={
    start(value){identity(value);phaseIs('Unstarted');phase='Building';pin=true;return 0;},
    issue(value,index){identity(value);phaseIs('Building');if(index!==issued||index>=count)throw new Error('issue-range');if(index&& !tables.published[index-1])throw new Error('unpublished');const nonce=issuer.mint();if(index===0)allocate();tables.nonce[index]=nonce;tables.generation[index]=1n;tables.live[index]=1;issued++;live++;return BigInt.asIntN(64,nonce);},
    publish(value,index,nonce){phaseIs('Building');owner(value,index,nonce);if(tables.published[index])throw new Error('double-publish');tables.published[index]=1;return 0;},
    complete(value){identity(value);phaseIs('Building');if(issued!==count||tables.published.some(value=>!value))throw new Error('unpublished');phase='Complete';pin=false;release();return 0;},
    borrow(value,index,nonce,view){phaseIs('Complete');owner(value,index,nonce);if(!Number.isInteger(view)||view<0||view>=64||tables.viewLive[view])throw new Error('view-capacity');if(tables.borrows[index]===0xffffffff)throw new Error('borrow-overflow');const token=issuer.mint();tables.viewNonce[view]=token;tables.viewSlot[view]=index;tables.viewGeneration[view]=tables.generation[index];tables.viewLive[view]=1;tables.borrows[index]++;return BigInt.asIntN(64,token);},
    read(value,view,nonce,relative){identity(value);phaseIs('Complete');if(!Number.isInteger(view)||view<0||view>=64||!tables.viewLive[view]||tables.viewNonce[view]!==BigInt.asUintN(64,nonce))throw new Error('stale-view');const index=tables.viewSlot[view];if(!tables.live[index]||tables.viewGeneration[view]!==tables.generation[index])throw new Error('stale-owner');if(!Number.isInteger(relative)||relative<0||relative>=width)throw new Error('read-range');return payload[index*width+relative];},
    end_borrow(value,view,nonce){identity(value);phaseIs('Complete');if(!Number.isInteger(view)||view<0||view>=64||!tables.viewLive[view]||tables.viewNonce[view]!==BigInt.asUintN(64,nonce))throw new Error('stale-view');const index=tables.viewSlot[view];if(!tables.live[index]||tables.generation[index]!==tables.viewGeneration[view]||!tables.borrows[index])throw new Error('stale-owner');tables.borrows[index]--;tables.viewLive[view]=0;return 0;},
    move_owner(value,index,nonce){phaseIs('Complete');owner(value,index,nonce);if(tables.borrows[index])throw new Error('borrowed');if(tables.generation[index]===0xffffffffffffffffn)throw new Error('generation-exhaustion');const token=issuer.mint();tables.generation[index]++;tables.nonce[index]=token;return BigInt.asIntN(64,token);},
    drop_owner(value,index,nonce){identity(value);if(phase!=='Complete'&&phase!=='Aborted')throw new Error('phase');slot(index,nonce);if(tables.borrows[index])throw new Error('borrowed');tables.live[index]=0;live--;release();return 0;},
    abort(value){identity(value);if(phase!=='Unstarted'&&phase!=='Building')throw new Error('phase');phase='Aborted';return 0;},
    settle_abort(value){identity(value);phaseIs('Aborted');if(live!==0||tables?.viewLive.some(value=>value))throw new Error('unsettled');pin=false;release();return 0;},
    phase(value){identity(value);return phaseCode[phase];},
  };
  // Imports never escape: this factory owns instantiation for exact granted
  // module identity; callers cannot attach the same closure to a foreign module.
  const instance=await WebAssembly.instantiate(grant,{[namespace]:Object.freeze(host)});
  return Object.freeze({api:instance.exports,observation:()=>({allocations,releases,metadataBytes,events,cumulative,pin,issued,live})});
}
const issuer=issuerOwner();
const first=await instantiate(module,issuer);const a=first.api;
a.start(issuer);assert.throws(()=>a.complete(issuer),/unpublished/);
assert.deepEqual(first.observation(),{allocations:0,releases:0,metadataBytes:0,events:0,cumulative:0,pin:true,issued:0,live:0});
const owners=[];
for(let index=0;index<count;index++){const nonce=a.issue(issuer,index);owners.push(nonce);if(index+1<count)assert.throws(()=>a.issue(issuer,index+1),/unpublished/);assert.throws(()=>a.borrow(issuer,index,nonce,0),/phase/);a.publish(issuer,index,nonce);}
a.complete(issuer);
assert.equal(first.observation().allocations,1);
assert.equal(first.observation().cumulative,count*width);
const view=a.borrow(issuer,count-1,owners.at(-1),0);
assert.equal(a.read(issuer,0,view,width-1),0);
assert.throws(()=>a.read(issuer,0,view,width),/read-range/);
assert.throws(()=>a.read(issuer,0,view,-1),/read-range/);
assert.throws(()=>a.move_owner(issuer,count-1,owners.at(-1)),/borrowed/);
assert.throws(()=>a.drop_owner(issuer,count-1,owners.at(-1)),/borrowed/);
assert.throws(()=>a.read(issuerOwner(),0,view,0),/foreign-issuer/);
a.end_borrow(issuer,0,view);assert.throws(()=>a.read(issuer,0,view,0),/stale-view/);
const concurrent=[];
for(let slot=0;slot<64;slot++)concurrent.push(a.borrow(issuer,0,owners[0],slot));
assert.throws(()=>a.borrow(issuer,0,owners[0],64),/view-capacity/);
assert.throws(()=>a.borrow(issuer,0,owners[0],0),/view-capacity/);
for(let slot=0;slot<64;slot++){assert.equal(a.read(issuer,slot,concurrent[slot],0),0);a.end_borrow(issuer,slot,concurrent[slot]);}

const old=owners[0];owners[0]=a.move_owner(issuer,0,old);
assert.throws(()=>a.drop_owner(issuer,0,old),/stale-owner/);
const other=await instantiate(module,issuer);other.api.start(issuer);
const foreign=other.api.issue(issuer,0);assert.throws(()=>a.drop_owner(issuer,0,foreign),/stale-owner/);
other.api.abort(issuer);assert.throws(()=>other.api.settle_abort(issuer),/unsettled/);
other.api.drop_owner(issuer,0,foreign);assert.equal(other.observation().releases,0);
other.api.settle_abort(issuer);assert.equal(other.observation().releases,1);
for(let index=0;index<count;index++){a.drop_owner(issuer,index,owners[index]);assert.equal(first.observation().releases,index===count-1?1:0);}
assert.equal(a.phase(issuer),phaseCode.Settled);
assert.equal(first.observation().events,1);
// Same nonce under a distinct issuer must never authenticate a foreign owner.
const separateIssuer=issuerOwner(), separate=await instantiate(module,separateIssuer);
separate.api.start(separateIssuer);const foreignIssuerNonce=separate.api.issue(separateIssuer,0);
assert.throws(()=>a.drop_owner(separateIssuer,0,foreignIssuerNonce),/foreign-issuer/);
separate.api.abort(separateIssuer);separate.api.drop_owner(separateIssuer,0,foreignIssuerNonce);separate.api.settle_abort(separateIssuer);
const required=count*22+64*21;
await assert.rejects(()=>instantiate(module,issuer,required-1),/metadata-preflight/);
const exact=await instantiate(module,issuer,required);exact.api.start(issuer);exact.api.abort(issuer);exact.api.settle_abort(issuer);
const duplicate=await WebAssembly.compile(bytes);
await assert.rejects(()=>instantiate(duplicate,issuer),/foreign-module/);
const reentry=await instantiate(module,issuer);reentry.api.start(issuer);const token=reentry.api.issue(issuer,0);
assert.throws(()=>reentry.api.publish(issuer,0,old),/stale-owner/);
reentry.api.abort(issuer);reentry.api.drop_owner(issuer,0,token);reentry.api.settle_abort(issuer);
console.log('private-fused-row-host-v1-ok');
