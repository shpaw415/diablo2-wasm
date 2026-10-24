data = open('/tmp/opencode/Javazon.d2s','rb').read()
HUFF = {' ':"10",'0':"11111011",'1':"1111100",'2':"001100",'3':"1101101",'4':"11111010",
'5':"00010110",'6':"1101111",'7':"01111",'8':"000100",'9':"01110",'a':"11110",'b':"0101",
'c':"01000",'d':"110001",'e':"110000",'f':"010011",'g':"11010",'h':"00011",'i':"1111110",
'j':"000101110",'k':"010010",'l':"11101",'m':"01101",'n':"001101",'o':"1111111",'p':"11001",
'q':"11011001",'r':"11100",'s':"0010",'t':"01100",'u':"00001",'v':"1101110",'w':"00000",
'x':"00111",'y':"0001010",'z':"11011000"}
DEC={v:k for k,v in HUFF.items()}
class R:
    def __init__(s,p): s.p=p
    def read(s,n):
        v=0
        for i in range(n):
            v |= ((data[s.p//8]>>(s.p%8))&1)<<i; s.p+=1
        return v
    def align(s): s.p=(s.p+7)&~7
    def huff4(s):
        out=""
        for _ in range(4):
            code=""
            while code not in DEC:
                code+=str((data[s.p//8]>>(s.p%8))&1); s.p+=1
                if len(code)>9: raise ValueError("huffman")
            out+=DEC[code]
        return out.rstrip(' ')

import os
A='/home/agent/workspace/d2w-integ/assets/'
def col_set(path,col):
    out=set()
    with open(path,encoding='cp1252') as fh:
        hdr=fh.readline().rstrip('\r\n').split('\t'); ci=hdr.index(col)
        for line in fh:
            f=line.rstrip('\r\n').split('\t')
            if len(f)>ci and f[ci].strip(): out.add(f[ci].strip())
    return out
def load(path,k,vcols):
    out={}
    with open(path,encoding='cp1252') as fh:
        hdr=fh.readline().rstrip('\r\n').split('\t'); idx={n:i for i,n in enumerate(hdr)}
        for n,line in enumerate(fh):
            f=line.rstrip('\r\n').split('\t'); f+=['']*(len(hdr)-len(f))
            out[n]={c:(f[idx[c]].strip() if c in idx else '') for c in vcols}
    return out
ARMOR=col_set(A+'patch_armor.txt','code'); WEAP=col_set(A+'patch_weapons.txt','code'); MISC=col_set(A+'patch_misc.txt','code')
UNIQ=load(A+'patch_uniqueitems.txt','index',['index','code'])
SETI=load(A+'patch_setitems.txt','index',['index','item'])
PREF=load(A+'patch_magicprefix.txt','Name',['Name'])
SUFF=load(A+'patch_magicsuffix.txt','Name',['Name'])
def load_isc(path):
    t={}
    with open(path,encoding='cp1252') as fh:
        hdr=fh.readline().rstrip('\r\n').split('\t'); idx={n:i for i,n in enumerate(hdr)}
        for line in fh:
            f=line.rstrip('\r\n').split('\t'); f+=['']*(len(hdr)-len(f))
            if not f[idx['Stat']].strip(): continue
            try: sid=int(f[idx['ID']])
            except: continue
            def num(n):
                v=f[idx[n]].strip() if n in idx else ''
                return int(v) if v else 0
            t[sid]={'stat':f[idx['Stat']].strip(),'sb':num('Save Bits'),'sa':num('Save Add')}
    return t
ISC=load_isc(A+'patch_itemstatcost.txt')
def base(name,tok):
    i=name.find(tok)
    return name[:i]+name[i+len(tok):] if i>=0 else None
def mod_width(sid):
    r=ISC.get(sid)
    if not r or not r['sb']: return None
    nxt=ISC.get(sid+1)
    if 'min' in r['stat'] and nxt and nxt['sb'] and 'max' in nxt['stat'] and base(r['stat'],'min')==base(nxt['stat'],'max'):
        return r['sb']+nxt['sb']
    return r['sb']
# flags has_dur/stacked par type: approx via MISC/WEAP/ARMOR
STACKED=set()
with open(A+'patch_misc.txt',encoding='cp1252') as fh:
    hdr=fh.readline().rstrip('\r\n').split('\t'); idx={n:i for i,n in enumerate(hdr)}
    ci=idx.get('code'); si=idx.get('stackable')
    for line in fh:
        f=line.rstrip('\r\n').split('\t'); f+=['']*(len(hdr)-len(f))
        if ci is not None and len(f)>ci and f[ci].strip() and si is not None and len(f)>si and f[si].strip()=='1':
            STACKED.add(f[ci].strip())
HASDUR=set()
with open(A+'patch_weapons.txt',encoding='cp1252') as fh:
    hdr=fh.readline().rstrip('\r\n').split('\t'); idx={n:i for i,n in enumerate(hdr)}
    ci=idx.get('code'); di=idx.get('durability')
    for line in fh:
        f=line.rstrip('\r\n').split('\t'); f+=['']*(len(hdr)-len(f))
        if ci is not None and len(f)>ci and f[ci].strip():
            d=f[di].strip() if di is not None and len(f)>di else ''
            if d and d!='0': HASDUR.add(f[ci].strip())
with open(A+'patch_armor.txt',encoding='cp1252') as fh:
    hdr=fh.readline().rstrip('\r\n').split('\t'); idx={n:i for i,n in enumerate(hdr)}
    ci=idx.get('code'); di=idx.get('durability')
    for line in fh:
        f=line.rstrip('\r\n').split('\t'); f+=['']*(len(hdr)-len(f))
        if ci is not None and len(f)>ci and f[ci].strip():
            d=f[di].strip() if di is not None and len(f)>di else ''
            if d and d!='0': HASDUR.add(f[ci].strip())

MODE={0:"stored",1:"equipped",2:"belt",4:"cursor",6:"socketed"}
POS={0:"-",1:"head",2:"neck",3:"torso",4:"weapon_R",5:"weapon_L",6:"ring_R",7:"ring_L",8:"belt",9:"feet",10:"hands",11:"swap_R",12:"swap_L"}
STO={-1:"-",0:"?",1:"inv",4:"cube",5:"stash"}
QN={1:"low",2:"normal",3:"sup",4:"magic",5:"set",6:"rare",7:"unique",8:"crafted"}

def read_mods(r,label=""):
    mods=[]
    while True:
        sid=r.read(9)
        if sid>=0x1ff: return mods
        rec=ISC.get(sid)
        if rec is None or rec['sb']==0:
            mods.append((sid,None,'??save_bits=0',0)); return mods
        w=mod_width(sid)
        if w is None: w=rec['sb']
        v=r.read(w)
        mods.append((sid,v,rec['stat'],w))

def parse_item(r,depth=0):
    ind="  "*depth
    start=r.p
    flags=r.read(32)
    id_,qtyf,sock,ear,newb,simple,eth,perso,rw,chr1,chr2 = (flags>>4&1,flags>>5&1,flags>>11&1,flags>>16&1,
        flags>>17&1,flags>>21&1,flags>>22&1,flags>>24&1,flags>>26&1,flags>>28&1,flags>>29&1)
    hf=r.read(1); vv=r.read(2); ver=(vv+99) if hf else vv
    mode=r.read(3)
    if mode in (3,5):
        col=r.read(16); row=r.read(16); pos=0; sto=-1
    else:
        pos=r.read(4); col=r.read(4); row=r.read(4); sto=r.read(3)-1
    loc=f"{MODE.get(mode,mode)}/{POS.get(pos,pos)}"
    if ear:
        cls=r.read(3); lvl=r.read(7); nm=""
        while True:
            c=r.read(7)
            if c==0: break
            nm+=chr(c)
        r.align()
        print(f"{ind}EAR classe={cls} lvl={lvl} nom={nm!r} {loc} @{start//8}.{start%8}")
        return
    code=r.huff4()
    if simple:
        # quest diff: aucun item quest attendu; realm flag
        if r.read(1):
            rd=[r.read(32) for _ in range(4)]
            print(f"{ind}SIMPLE {code:5} id={id_} qtyflag={qtyf} {loc}/{STO.get(sto,sto)} ver={ver} REALM={rd} @{start//8}.{start%8}")
        else:
            print(f"{ind}SIMPLE {code:5} id={id_} qtyflag={qtyf} {loc}/{STO.get(sto,sto)} ver={ver} @{start//8}.{start%8}")
        r.align(); return
    gems=r.read(3)
    guid=r.read(32); ilvl=r.read(7); q=r.read(4)
    extra=""
    if r.read(1): r.read(3)
    if r.read(1): r.read(11)
    if q==1: r.read(3)
    elif q==3: r.read(3)
    elif q==4:
        p=r.read(11); s=r.read(11)
        extra=f" pref[{p}]={PREF.get(p,{}).get('Name','?')} suff[{s}]={SUFF.get(s,{}).get('Name','?')}"
    elif q==5:
        sid=r.read(12); extra=f" set[{sid}]={SETI.get(sid,{}).get('index','?')}"
    elif q in (6,8):
        n1,n2=r.read(8),r.read(8); aff=[]
        for _ in range(6):
            if r.read(1): aff.append(r.read(11))
        extra=f" ({n1},{n2}) aff={aff}"
    elif q==7:
        sid=r.read(12); extra=f" UNIQUE[{sid}]={UNIQ.get(sid,{}).get('index','?')}"
    rwid=r.read(16) if rw else None
    if rwid is not None: extra+=f" RWid={rwid}"
    if perso:
        nm=""
        while True:
            c=r.read(7)
            if c==0: break
            nm+=chr(c)
        extra+=f" nom={nm!r}"
    if r.read(1):
        rd=[r.read(32) for _ in range(4)]
        extra+=f" REALM={[hex(x) for x in rd]}"
    td=""
    is_armor=code in ARMOR; is_weap=code in WEAP
    if is_armor:
        dv=r.read(11); td=f" def={dv-10}"
        if code in HASDUR:
            mx=r.read(8); du=r.read(9) if mx>0 else None
            td+=f" dur={du}/{mx}"
    elif is_weap:
        if code in HASDUR:
            mx=r.read(8); du=r.read(9) if mx>0 else None
            td=f" dur={du}/{mx}"
        else:
            td=" (sans dur)"
    # quantité (ordre D2R: après dur, avant sockets)
    if ver>=0x69:
        if r.read(1): td+=f" qty={r.read(9)}"
    elif code in STACKED:
        td+=f" qty={r.read(9)}"
    if sock:
        td+=f" sockN={r.read(4)}"
    setflags=[r.read(1) for _ in range(5)] if q==5 else None
    if q==5: td+=f" setflags={setflags}"
    mods=read_mods(r)
    ms=" ".join(f"{v:+d} {st}(id{i})" for i,v,st,w in mods if v is not None)
    bad=" !!"+",".join(str(i) for i,v,st,w in mods if v is None)
    if q==5 and setflags:
        for f_ in setflags:
            if f_: read_mods(r)
    if rw:
        rwm=read_mods(r)
        ms=" RW["+" ".join(f"{v:+d} {st}(id{i})" for i,v,st,w in rwm if v is not None)+"] "+ms
    print(f"{ind}{code:5} ilvl={ilvl:2} guid={guid:#010x} q={QN.get(q,q):6}{' ETH' if eth else ''}{' perso' if perso else ''}{' sock' if sock else ''}{td}{extra} mods=[{ms}{bad}] {loc}/{STO.get(sto,sto)} x={col} y={row} ver={ver}{' CHRON' if chr1 else ''} gems={gems} @{start//8}.{start%8}")
    r.align()

def parse_list(r,label):
    jm=bytes(r.read(8) for _ in range(2))
    assert jm==b"JM", f"{label}: {jm}"
    cnt=r.read(16)
    print(f"--- {label}: {cnt} items @byte {r.p//8}")
    for i in range(cnt):
        parse_item(r)
    return cnt
r=R(848*8)
parse_list(r,"character")
p=r.p//8
print("fin:",p,"| suite:",data[p:p+6])
if data[p:p+2]==b"JM": parse_list(r,"corpse")
p=r.p//8
if data[p:p+2]==b"jf":
    r.read(16); parse_list(r,"merc"); p=r.p//8
if data[p:p+2]==b"kf":
    r.read(16); g=r.read(8); print("golem:",g)
    if g: parse_list(r,"golem")
p=r.p//8
print("FIN:",p,"/",len(data), "OK ✓" if p==len(data) else "DÉPASSEMENT")
