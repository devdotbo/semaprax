import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const {instance}=await WebAssembly.instantiate(readFileSync('program.wasm'),{env:{
 spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,
 spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{throw Error('contract');},
}});
const e=instance.exports, bytes=new Uint8Array(e.memory.buffer);
const symbol=id=>'spx_text_'+Buffer.from(id).toString('hex');
const same=e[symbol('text.same')],different=e[symbol('text.different')];
const status=()=>e.__spx_text_status_v1.value;
for(let repeat=0;repeat<3;repeat++){
 bytes.set([0xc3,0xa9,0,0x78],16);bytes.set([0xc3,0xa9,0,0x78],64);
 assert.equal(same(16,4,64,4),1);assert.equal(different(16,4,64,4),0);assert.equal(status(),0);
 assert.equal(same(16,4,16,2),0);assert.equal(different(16,4,16,2),1);
 bytes[67]=0x79;assert.equal(same(16,4,64,4),0);assert.equal(different(16,4,64,4),1);
 assert.equal(same(16,0,64,0),1);
 // Both roots are validated even when length/prefix makes equality false.
 bytes[128]=0x80;assert.equal(same(16,0,128,1),0);assert.equal(status(),2);
 assert.equal(different(16,0,65535,2),0);assert.equal(status(),1);
 bytes.fill(0x61,0,65536);assert.equal(same(0,32768,32768,32768),1);assert.equal(status(),0);
 assert.equal(same(0,32769,32768,32768),0);assert.equal(status(),1);
 assert.equal(same(0,1,16,1),1);assert.equal(status(),0);
}
