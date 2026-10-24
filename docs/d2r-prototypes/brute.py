import itertools
data = open('/tmp/opencode/Javazon.d2s','rb').read()

class R:
    def __init__(s, p): s.p=p
    def read(s,n):
        v=0
        for i in range(n):
            b=data[(s.p+i)//8]; v |= ((b>>((s.p+i)%8))&1)<<i
        s.p+=n; return v
    def ascii(s,n): return bytes(s.read(8) for _ in range(n)).decode('ascii','replace')
    def align(s): s.p=(s.p+7)&~7

# variantes sur un item compact (famille 1 @852)
for ver,yb,cb,sk,al in itertools.product([0,3],[3,4],[24,32],[0,1],[0,1]):
    r=R(852*8)
    flags=[r.read(1) for _ in range(32)]
    if ver: v=r.read(ver)
    par=r.read(3); eq=r.read(4); x=r.read(4); y=r.read(yb); st=r.read(3)
    code=r.ascii(cb).rstrip('\x00 ')
    if sk: s1=r.read(1)
    if al: r.align()
    printable = all(32 <= ord(c) < 127 for c in code) if code else False
    end=r.p//8
    print(f"ver={ver} y={yb} code={cb} sock={sk} align={al} -> code={code!r} ascii={printable} fin_byte={end} par={par} eq={eq} x={x} y={y} st={st} flags[4,11,16,21,22,24,26]={flags[4]},{flags[11]},{flags[16]},{flags[21]},{flags[22]},{flags[24]},{flags[26]}")
