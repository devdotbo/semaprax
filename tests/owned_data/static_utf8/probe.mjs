import {readFile} from 'node:fs/promises';
const module = await WebAssembly.compile(await readFile('probe.wasm'));
let instance;
const decode = c => { const w=BigInt.asUintN(64,c); return {p:Number(w>>32n),n:Number(w&0xffffffffn)}; };
const read = c => { const {p,n}=decode(c),memory=new Uint8Array(instance.exports.__spx_byte_memory.buffer);
  if(p<196608||n>65536||p>262144-n)throw Error('not authenticated static interval');
  return memory.subarray(p,p+n); };
const fail = name => () => {throw Error(`unexpected constructor/import ${name}`)};
const env = Object.fromEntries(WebAssembly.Module.imports(module).map(i=>[i.name,fail(i.name)]));
Object.assign(env,{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
  spx_bytes_get:(c,i)=>{const b=read(c);return i<0n||i>=BigInt(b.length)?-1:b[Number(i)]},spx_bytes_as_slice:c=>{read(c);return c}});
({instance}=await WebAssembly.instantiate(module,{env}));
const memory=new Uint8Array(instance.exports.__spx_byte_memory.buffer);
if(memory.length!==262144)throw Error('selected fixed memory');
const pool=memory.slice(196608);
for(let i=0;i<4;i++){if(instance.exports.semaprax_main()!==42n)throw Error('semantic mismatch');
  if(!pool.every((byte,j)=>byte===memory[196608+j]))throw Error('literal pool mutated');}
let grew=false;try{instance.exports.__spx_byte_memory.grow(1);grew=true}catch{}
if(grew)throw Error('pool memory grew');
