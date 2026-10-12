const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectedValue=BigInt(process.argv[3]),refuse=process.argv[4]==='1';
let next=1n;const entries=new Map();
const slot=h=>BigInt.asUintN(64,h)&0xffffffffn;
const read=(h,tag)=>{const v=entries.get(slot(h));if(typeof h!=='bigint'||h===0n||!v||v.tag!==tag||tag!==1||v.generation!==(BigInt.asUintN(64,h)>>32n))throw Error('stale or mistyped Copy record carrier');return v};
const alloc=(tag,capacity,values=[])=>{const h=next++;entries.set(h,{tag,capacity,values,generation:0n,identity:undefined,shape:undefined});return h};
const move=(h,v)=>{if(!entries.delete(slot(h)))throw Error('double move');const fresh=alloc(v.tag,v.capacity,v.values),out=entries.get(fresh);out.identity=v.identity;out.shape=v.shape;return fresh};
const totalKey=(word,tag)=>{let b=BigInt.asUintN(64,word);if(tag===1)return b^0x8000000000000000n;if(tag===2)return (b&0xffffffffn)^0x80000000n;if(tag===6){b&=0xffffffffn;return b&0x80000000n?(~b&0xffffffffn):b^0x80000000n;}if(tag===7)return b&0x8000000000000000n?~b&0xffffffffffffffffn:b^0x8000000000000000n;return b;};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:c=>{throw Error('status:'+c)},
spx_vec_with_capacity:(t,n)=>!refuse&&t===1&&n>=0n&&n<=8192n?alloc(t,Number(n)):0n,
spx_vec_push:(h,t,b)=>{const v=read(h,t);if(v.values.length===v.capacity)return 0n;v.values.push(b);return move(h,v)},
spx_vec_len:(h,t)=>BigInt(read(h,t).values.length),spx_vec_capacity:(h,t)=>BigInt(read(h,t).capacity),
spx_vec_get:(h,t,i)=>{const v=read(h,t);if(i<0n||i>=BigInt(v.values.length))throw Error('unchecked get');return v.values[Number(i)]},
spx_vec_drop:h=>{if(!entries.delete(slot(h)))throw Error('double drop')},
spx_vec_reserve_exact:(h,t,n)=>{const v=read(h,t),needed=BigInt(v.values.length)+n;if(n<0n||needed>8192n)return 0n;v.capacity=Math.max(v.capacity,Number(needed));return move(h,v)},
spx_vec_set:(h,t,i,b)=>{const v=read(h,t);if(i<0n||i>=BigInt(v.values.length))return 0n;v.values[Number(i)]=b;return move(h,v)},
spx_vec_clear:(h,t)=>{const v=read(h,t);v.values=[];return move(h,v)},
spx_vec_sort_v3:()=>{throw Error('record ordering must use checked field types')},
spx_vec_sort_by_v1:(h,identity,shape,policy)=>{
 const v=read(h,1),rows=v.values;let width=0;
 for(let raw=BigInt.asUintN(64,shape);raw;raw>>=8n){const f=Number(raw&255n);if((f>>4)<1||(f>>4)>8||(f&15)!==width)throw Error('Copy sort layout');width++;}
 if(!width||width>8||rows.length%width||v.capacity%width)throw Error('Copy sort stride');
 if(policy===0n){if(v.identity!==undefined&&(v.identity!==identity||v.shape!==shape))throw Error('Copy sort rebound');v.identity=identity;v.shape=shape;return h;}
 if(v.identity!==identity||v.shape!==shape)throw Error('foreign Copy sort descriptor');
 let seen=0,count=0;for(let raw=BigInt.asUintN(64,policy);raw;raw>>=8n){const f=(Number(raw&255n)>>1)-1;if(f<0||f>=width||(seen&(1<<f)))throw Error('Copy sort keys');seen|=1<<f;count++;}
 if(!count||count>width||v.generation===0xffffffffn)throw Error('Copy sort key count/epoch');
 const inventory=entries.size;
 for(let i=1;i<rows.length/width;i++){let j=i;while(j){let order=0;
  for(let raw=BigInt.asUintN(64,policy);raw&&!order;raw>>=8n){const byte=Number(raw&255n),f=(byte>>1)-1,tag=Number((BigInt.asUintN(64,shape)>>BigInt(8*f+4))&15n);const a=totalKey(rows[(j-1)*width+f],tag),b=totalKey(rows[j*width+f],tag);order=byte&1?(b>a?1:b<a?-1:0):(a>b?1:a<b?-1:0);}
  if(order<=0)break;for(let f=0;f<width;f++){const word=rows[(j-1)*width+f];rows[(j-1)*width+f]=rows[j*width+f];rows[j*width+f]=word;}j--;
 }}
 v.generation++;if(rows!==v.values||entries.size!==inventory)throw Error('Copy sort replaced storage/authority');return BigInt.asIntN(64,(v.generation<<32n)|slot(h));
},
// Borrowed field uniqueness: exact bound facts, ordinal scratch only, no renewal.
spx_vec_unique_by_v1:(h,identity,shape,position)=>{
 const v=read(h,1),rows=v.values;let width=0;
 if(v.identity!==identity||v.shape!==shape)throw Error('foreign Copy uniqueness descriptor');
 for(let raw=BigInt.asUintN(64,shape);raw;raw>>=8n){const f=Number(raw&255n);if((f>>4)<1||(f>>4)>8||(f&15)!==width)throw Error('Copy uniqueness layout');width++;}
 if(!width||width>8||rows.length%width||v.capacity%width||position<0n||position>=BigInt(width))throw Error('Copy uniqueness position');
 const tag=Number((BigInt.asUintN(64,shape)>>(position*8n+4n))&15n),f=Number(position),n=rows.length/width;
 if(tag===6||tag===7)throw Error('Copy uniqueness key outside v1 profile');
 if(n<2)return 1;if(refuse)return 3;
 const inventory=entries.size,generation=v.generation,o=new Uint32Array(n);for(let i=0;i<n;i++)o[i]=i;
 const cmp=(a,b)=>{const x=totalKey(rows[a*width+f],tag),y=totalKey(rows[b*width+f],tag);return x<y?-1:x>y?1:0};
 const sift=(r,m)=>{for(;;){let c=2*r+1;if(c>=m)return;if(c+1<m&&cmp(o[c],o[c+1])<0)c++;if(cmp(o[r],o[c])>=0)return;const t=o[r];o[r]=o[c];o[c]=t;r=c;}};
 for(let i=Math.floor(n/2);i-->0;)sift(i,n);for(let e=n-1;e>0;e--){const t=o[0];o[0]=o[e];o[e]=t;sift(0,e);}
 let unique=1;for(let i=1;i<n&&unique;i++)if(cmp(o[i-1],o[i])===0)unique=0;
 if(rows!==v.values||entries.size!==inventory||v.generation!==generation)throw Error('Copy uniqueness changed storage/authority');return unique;
}};
(async()=>{
 if(WebAssembly.Module.imports(new WebAssembly.Module(bytes)).some(entry=>entry.name==='spx_vec_sort_by_v1')){
  const missing={...env};delete missing.spx_vec_sort_by_v1;let refused=false;
  try{await WebAssembly.instantiate(bytes,{env:missing})}catch(error){if(!(error instanceof WebAssembly.LinkError))throw error;refused=true;}
  if(!refused)throw Error('explicit Copy sort linked without private import');
 }
 if(WebAssembly.Module.imports(new WebAssembly.Module(bytes)).some(entry=>entry.name==='spx_vec_unique_by_v1')){
  const missing={...env};delete missing.spx_vec_unique_by_v1;let refused=false;
  try{await WebAssembly.instantiate(bytes,{env:missing})}catch(error){if(!(error instanceof WebAssembly.LinkError))throw error;refused=true;}
  if(!refused)throw Error('Copy field uniqueness linked without its distinct private import');
 }
 return WebAssembly.instantiate(bytes,{env});
})().then(({instance})=>{for(let n=0;n<3;n++){let result,selected=0;try{result=instance.exports.semaprax_main()}catch(e){if(!e.message.startsWith('status:'))throw e;selected=Number(e.message.slice(7))}if(selected!==expected||(!expected&&result!==expectedValue)||entries.size!==0)throw Error(`result/settlement ${selected}/${result}/${entries.size}`)}}).catch(e=>{console.error(e);process.exit(2)});
