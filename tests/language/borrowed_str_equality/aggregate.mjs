import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
let memory,next=1,allocations=0,drops=0,comparisons=0;
const owners=new Map(), fail=()=>{throw Error('unexpected host call');};
const span=carrier=>{
 const bits=BigInt.asUintN(64,carrier), pointer=Number(bits>>32n), length=Number(bits&0xffffffffn);
 if(pointer&0x80000000){const value=owners.get(pointer&0x7fffffff);if(!value||value.length!==length)throw Error('stale owner');return value;}
 if(pointer>memory.buffer.byteLength||length>memory.buffer.byteLength-pointer)throw Error('range');
 return new Uint8Array(memory.buffer,pointer,length);
};
const checked=v=>{if(v < -9223372036854775808n||v>9223372036854775807n)throw Error('overflow');return v;};
const {instance}=await WebAssembly.instantiate(readFileSync('program.wasm'),{env:{
 spx_add:(a,b)=>checked(a+b),spx_sub:(a,b)=>checked(a-b),spx_mul:(a,b)=>checked(a*b),
 spx_div:(a,b)=>checked(a/b),spx_rem:(a,b)=>checked(a%b),spx_neg:a=>checked(-a),
 spx_contract_fail:status=>{throw Error(`status:${status}`);},
 spx_bytes_copy:c=>{const value=Uint8Array.from(span(c)),id=next++;owners.set(id,value);allocations++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(value.length));},
 spx_bytes_drop:c=>{span(c);const id=Number(BigInt.asUintN(64,c)>>32n)&0x7fffffff;if(!owners.delete(id))throw Error('double drop');drops++;},
 spx_bytes_as_slice:c=>{span(c);return c;},
 // Authenticate the carrier even when the requested index is absent. The
 // unsigned i64 index is the exact byte_get ABI; -1 is Option::None.
 spx_bytes_get:(c,index)=>{const value=span(c),at=BigInt.asUintN(64,index);return at>=BigInt(value.length)?-1:value[Number(at)];},
 spx_string_concat_v1:fail,spx_string_from_char_v1:fail,spx_string_len_chars_v1:fail,
 spx_string_from_i64_v1:fail,spx_string_from_usize_v1:fail,spx_string_contains_v1:fail,
 spx_string_starts_with_v1:(a,b)=>{const left=span(a),right=span(b);comparisons++;return right.length<=left.length&&right.every((v,i)=>left[i]===v)?1:0;},
}});
memory=instance.exports.__spx_byte_memory||instance.exports.memory;
for(let repeat=0;repeat<3;repeat++){
 const before=allocations,compared=comparisons;
 if(process.argv[2]==='failure')assert.throws(()=>instance.exports.semaprax_main(),/status:10/);
 else assert.equal(instance.exports.semaprax_main(),42n);
 const expected=Number(process.argv[3]);
 assert.equal(allocations-before,expected);assert.equal(comparisons-compared,expected===0?0:6);
 assert.equal(owners.size,0);assert.equal(allocations,drops);
}
