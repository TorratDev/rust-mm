#!/usr/bin/env python3
"""#270 P1 probe: does the message display 02154BF0 advance the global LCG (sPRNGState 0x020DF704)?
Usage: python msg_lcg_probe.py ROM OUT.json NFRAMES NTH POKE40
  e.g. python msg_lcg_probe.py rom_copy.nds out.json 36000 10 0x4a
ROM is a scratch-named copy of pmdb.nds outside the repo; OUT.json is outside the repo too.
Inputs: boot_to_dungeon.txt, then combat_inputs.txt from the dungeon frame. Every hook is gated on the
dungeon struct being present (other overlays share these addresses before the dungeon).
FORCED, not natural: from the NTH-th RunMonsterAI entry on, pokes info+0xd9=1 and info+0x40=POKE40 so that
step 4 calls 02154BF0 at 021880d4; info+0x40 is cleared on return. Logs entry/return LCG and LCG writes."""
import os, sys, json
os.environ.setdefault("SDL_VIDEODRIVER","dummy"); os.environ.setdefault("SDL_AUDIODRIVER","dummy")
V=os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0,V)
from desmume.emulator import DeSmuME
from retail_trace import parse_inputs, Ram, G_DUNGEON, S_PRNG
rom,out,nframes,nth,poke40=sys.argv[1],sys.argv[2],int(sys.argv[3]),int(sys.argv[4]),int(sys.argv[5],0)
emu=DeSmuME(); emu.open(rom); ram=Ram(emu); R=emu.memory.register_arm9
mem=emu.memory
boot=parse_inputs(V+"/boot_to_dungeon.txt"); turns=parse_inputs(V+"/combat_inputs.txt")
fr=[0]; dfr=[None]; ev=[]; st={"ai":0,"in_ai":False,"depth":0,"forced":False}
def reg(n): return int(getattr(R,n))&0xFFFFFFFF
def live(): return dfr[0] is not None
def lcg(): return ram.u32(S_PRNG)
def log(*a): ev.append([fr[0],*a])
def on_ai(*_):
    if not live(): return
    st["ai"]+=1
    if st["ai"]>=nth and not st.get("done"):
        e=reg("r0"); info=ram.u32(e+0x70)
        log("FORCE_poke",hex(e),hex(info),"info98",ram.u8(info+0x98),"info6",ram.u8(info+6),"d9_was",ram.u8(info+0xd9),"40_was",ram.u8(info+0x40))
        mem.write_byte(info+0xd9,1)
        if poke40: mem.write_byte(info+0x40,poke40)
        st["forced"]=True; st["e"]=e; st["info"]=info
        st["watch"]=True
def on_bf0(*_):
    if not live(): return
    lr=reg("r14"); log("bf0_entry",hex(lr),"r0",hex(reg("r0")),"r1",hex(reg("r1")),"lcg",hex(lcg()))
    st['n910_at']=st.get('n910',0); st["bf0_lcg"]=lcg(); st["bf0_lr"]=lr; st["depth"]=1
    mem.register_exec(lr,on_ret)
def on_ret(*_):
    if st["depth"]:
        st["depth"]=0; mem.write_byte(st['info']+0x40,0)
        log("bf0_return","n910_inside",st.get("n910",0)-st.get("n910_at",0),"lcg_before",hex(st["bf0_lcg"]),"lcg_after",hex(lcg()),"changed",st["bf0_lcg"]!=lcg())
def on_910(*_):
    st['n910']=st.get('n910',0)+1
    if live() and st.get("watch"): log("910_entry",hex(reg("r14")),"lcg",hex(lcg()))
def on_w(addr,size):
    if live() and st.get("watch"): log("lcg_write",hex(reg("r15")),"depth",st["depth"],"val",hex(ram.u32(S_PRNG)))
for a,f in ((0x02188030,on_ai),(0x02154BF0,on_bf0),(0x02154910,on_910),(0x0213AFA8,lambda *_: live() and st.get("watch") and log("AFA8_entry")),(0x0213AD54,lambda *_: live() and st.get("watch") and log("AD54_entry"))):
    mem.register_exec(a,f)
mem.register_write(S_PRNG,on_w,4)
f=0
while f<nframes:
    fr[0]=f
    if dfr[0] is None:
        if f in boot: emu.input.keypad_update(boot[f])
    else:
        t=f-dfr[0]
        if t in turns: emu.input.keypad_update(turns[t])
    emu.cycle(with_joystick=False)
    if dfr[0] is None and f%10==0 and ram.u32(G_DUNGEON)!=0: dfr[0]=f
    f+=1
json.dump({"dungeon_frame":dfr[0],"events":ev,"n":len(ev)},open(out,"w"),indent=0)
print(len(ev))
emu.destroy()
