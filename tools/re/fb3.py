# Extract FireBullets3 constant args per weapon from `objdump -d -C --no-show-raw-insn cs.so` output.
# Usage: python3 -I fb3.py cs.asm
import re,struct,sys
lines=open(sys.argv[1]).read().split('\n')
fn=None; start=0; out={}
for i,l in enumerate(lines):
    m=re.match(r'^[0-9a-f]+ <(.*)>:$',l)
    if m: fn=m.group(1); start=i; continue
    if 'call' in l and 'FireBullets3' in l and fn and 'Fire(' in fn:
        regs={}; slots={}
        for l2 in lines[max(start,i-120):i]:
            ins=l2.split('\t')[-1].strip() if '\t' in l2 else ''
            a=re.match(r'mov\s+\$0x([0-9a-f]+),%(\w+)$',ins)
            if a: regs[a.group(2)]=int(a.group(1),16); continue
            a=re.match(r'xor\s+%(\w+),%\1$',ins)
            if a: regs[a.group(1)]=0; continue
            a=re.match(r'movl\s+\$0x([0-9a-f]+),0x([0-9a-f]+)\(%esp\)$',ins)
            if a: slots[int(a.group(2),16)]=int(a.group(1),16); continue
            a=re.match(r'mov\s+%(\w+),0x([0-9a-f]+)\(%esp\)$',ins)
            if a: slots[int(a.group(2),16)]=regs.get(a.group(1)); continue
            a=re.match(r'mov\s+.*,%(\w+)$',ins)
            if a: regs.pop(a.group(1),None)
        f=lambda v: round(struct.unpack('<f',struct.pack('<I',v))[0],3) if v is not None else None
        name=fn.split('(')[0]
        out.setdefault(name,[]).append((f(slots.get(0x14)),slots.get(0x18),slots.get(0x1c),slots.get(0x20),f(slots.get(0x24)),slots.get(0x2c)))
for k,vs in out.items():
    for v in vs: print(f"{k:28} dist={v[0]} pen={v[1]} bullet={v[2]} dmg={v[3]} rangemod={v[4]}")
