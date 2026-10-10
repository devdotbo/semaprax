import {readFile} from 'node:fs/promises';
const module=await WebAssembly.compile(await readFile('probe.wasm'));
let instance,next=1,allocations=0,drops=0,writes=0;
const owners=new Map();
const decode=c=>{const w=BigInt.asUintN(64,c);return {p:Number(w>>32n),n:Number(w&0xffffffffn)}};
const read=c=>{const d=decode(c);if((d.p&0x80000000)!==0){const bytes=owners.get(d.p&0x7fffffff);if(!bytes||bytes.length!==d.n)throw Error('owner');return bytes;}
  if(d.p<196608||d.n>65536||d.p>262144-d.n)throw Error('static range');
  return new Uint8Array(instance.exports.__spx_byte_memory.buffer,d.p,d.n);};
const fail=name=>()=>{throw Error(`unexpected constructor/import ${name}`)};
const env=Object.fromEntries(WebAssembly.Module.imports(module).map(i=>[i.name,fail(i.name)]));
Object.assign(env,{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
 spx_bytes_zeroed:n=>{if(n!==6n||owners.size!==0)throw Error('allocation shape');const id=next++;owners.set(id,new Uint8Array(6));allocations++;return BigInt.asIntN(64,(BigInt((id|0x80000000)>>>0)<<32n)|6n);},
 spx_bytes_set:(c,i,b)=>{const d=decode(c);if((d.p&0x80000000)===0)throw Error('write to immutable literal');const bytes=read(c);if(i<0n||i>=BigInt(bytes.length))throw Error('write bounds');bytes[Number(i)]=b;writes++;return c;},
 spx_bytes_get:(c,i)=>{const bytes=read(c);return i<0n||i>=BigInt(bytes.length)?-1:bytes[Number(i)]},
 spx_bytes_as_slice:c=>{read(c);return c;},
 spx_bytes_drop:c=>{const d=decode(c);if((d.p&0x80000000)===0||!owners.delete(d.p&0x7fffffff))throw Error('drop authority');drops++;}});
({instance}=await WebAssembly.instantiate(module,{env}));
const memory=new Uint8Array(instance.exports.__spx_byte_memory.buffer),pool=memory.slice(196608);
if(memory.length!==262144)throw Error('fixed memory');
for(let i=0;i<4;i++){const a=allocations,d=drops,w=writes;
 if(instance.exports.semaprax_main()!==42n)throw Error('six byte oracle');
 if(allocations-a!==1||drops-d!==1||writes-w!==8||owners.size!==0)throw Error('perkey allocation or owner leak');
 if(!pool.every((b,j)=>b===memory[196608+j]))throw Error('immutable pool write');}
