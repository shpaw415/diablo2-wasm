import struct
data = open('/tmp/opencode/Javazon.d2s','rb').read()

HUFF = {' ':"10",'0':"11111011",'1':"1111100",'2':"001100",'3':"1101101",'4':"11111010",
'5':"00010110",'6':"1101111",'7':"01111",'8':"000100",'9':"01110",'a':"11110",'b':"0101",
'c':"01000",'d':"110001",'e':"110000",'f':"010011",'g':"11010",'h':"00011",'i':"1111110",
'j':"000101110",'k':"010010",'l':"11101",'m':"01101",'n':"001101",'o':"1111111",'p':"11001",
'q':"11011001",'r':"11100",'s':"0010",'t':"01100",'u':"00001",'v':"1101110",'w':"00000",
'x':"00111",'y':"0001010",'z':"11011000"}
DEC = {v:k for k,v in HUFF.items()}

class R:
    def __init__(s, p): s.p=p
    def read(s,n):
        v=0
        for i in range(n):
            b=data[(s.p+i)//8]; v |= ((b>>((s.p+i)%8))&1)<<i
        s.p+=n; return v
    def reads(s,n):
        v=s.read(n)
        if v >= 1<<(n-1): v -= 1<<n
        return v
    def align(s): s.p=(s.p+7)&~7
    def huffman(s, maxchars=4):
        out=""
        for _ in range(maxchars):
            code=""
            while code not in DEC:
                code += str(s.read(1))
                if len(code) > 9: raise ValueError("huffman overflow "+code)
            c = DEC[code]
            out += c
            if c == ' ': break
        return out.rstrip(' ')

def load_table(path, keycol, valcols):
    out={}
    with open(path, encoding='cp1252') as fh:
        hdr=fh.readline().rstrip('\r\n').split('\t')
        idx={n:i for i,n in enumerate(hdr)}
        for n,line in enumerate(fh):
            f=line.rstrip('\r\n').split('\t'); f+=['']*(len(hdr)-len(f))
            out[n]={c:(f[idx[c]].strip() if c in idx and idx[c]<len(f) else '') for c in valcols}
    return out
def col_set(path, col):
    out=set()
    with open(path, encoding='cp1252') as fh:
        hdr=fh.readline().rstrip('\r\n').split('\t')
        ci=hdr.index(col)
        for line in fh:
            f=line.rstrip('\r\n').split('\t')
            if len(f)>ci and f[ci].strip(): out.add(f[ci].strip())
    return out
A='/home/agent/workspace/d2w-integ/assets/'
ARMOR=col_set(A+'patch_armor.txt','code'); WEAP=col_set(A+'patch_weapons.txt','code'); MISC=col_set(A+'patch_misc.txt','code')
UNIQ=load_table(A+'patch_uniqueitems.txt','index',['index','code'])
SETI=load_table(A+'patch_setitems.txt','index',['index','item'])
PREF=load_table(A+'patch_magicprefix.txt','Name',['Name'])
SUFF=load_table(A+'patch_magicsuffix.txt','Name',['Name'])
def load_isc(path):
    t={}
    with open(path, encoding='cp1252') as fh:
        hdr=fh.readline().rstrip('\r\n').split('\t')
        idx={n:i for i,n in enumerate(hdr)}
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
STACKABLE={"tbk","ibk","key","tsc","isc","vps","yps","aqv","cqv","tkf","tax","bkf","bal","9tk","9ta","9bk","8lx","7bk","7tk","9ts","7ts","jav","pil","ssp","glv","tsp","9ja","7ja","7pi","7s7","7gl","7ts","am5","ama","amf","hpo","hp1","hp2","hp3","hp4","hp5","mp1","mp2","mp3","mp4","mp5","rps","bps","rvs","rvl","gps","ops","gpm","opl","gpl","vps","wms","elx","pk1","pk2","pk3"}

PAR={0:"stored",1:"equipped",2:"belt",4:"cursor",6:"socketed"}
STO={0:"none",1:"inventory",4:"cube",5:"stash"}
EQ={0:"-",1:"head",2:"neck",3:"torso",4:"weapon_R",5:"weapon_L",6:"ring_R",7:"ring_L",8:"belt",9:"feet",10:"hands",11:"swap_R",12:"swap_L"}
QN={1:"low",2:"normal",3:"sup",4:"magic",5:"set",6:"rare",7:"unique",8:"crafted"}

def read_mods(r):
    mods=[]
    while True:
        sid=r.read(9)
        if sid==0x1ff: return mods
        w=mod_width(sid)
        if w is None:
            mods.append((sid,None,None,None)); return mods
        mods.append((sid, r.reads(w), ISC.get(sid,{}).get('stat','?'), w))

def parse_item(r, depth=0):
    ind="  "*depth
    start=r.p
    flags=[r.read(1) for _ in range(32)]
    identified, socketed, ear, simple, ethereal = flags[4],flags[11],flags[16],flags[21],flags[22]
    personalized, runeword = flags[24],flags[26]
    ver=r.read(3); parent=r.read(3); equip=r.read(4); x=r.read(4); y=r.read(4); stored=r.read(3)
    unk10=r.read(10)
    desc=f"par={PAR.get(parent,parent)}/eq={EQ.get(equip,equip)}/x={x}/y={y}/{STO.get(stored,stored)} ver={ver} unk10={unk10:#012b}"
    if ear:
        cls=r.read(3); lvl=r.read(7)
        nm=""
        while True:
            c=r.read(8)
            if c==0: break
            nm+=chr(c)
        print(f"{ind}EAR class={cls} lvl={lvl} name={nm!r} {desc} @{start//8}.{start%8}")
        r.align(); return
    code=r.huffman()
    sockcnt = r.read(1) if simple else r.read(3)
    if simple:
        print(f"{ind}SIMPLE {code:5} id={identified} {desc} @{start//8}.{start%8}")
        r.align(); return
    fid=r.read(32); ilvl=r.read(7); q=r.read(4)
    extra=""
    if r.read(1): r.read(3)
    if r.read(1): r.read(11)
    if q==1: r.read(3)
    elif q==3: r.read(3)
    elif q==4:
        pfx=r.read(11); sfx=r.read(11)
        extra=f" q=magic pref[{pfx}]={PREF.get(pfx,{}).get('Name','?')} suff[{sfx}]={SUFF.get(sfx,{}).get('Name','?')}"
    elif q==5:
        sid=r.read(12); extra=f" q=set[{sid}]={SETI.get(sid,{}).get('index','?')}"
    elif q in (6,8):
        n1,n2=r.read(8),r.read(8); aff=[]
        for _ in range(6):
            if r.read(1): aff.append(r.read(11))
        extra=f" q=rare ({n1},{n2}) aff={aff}"
    elif q==7:
        sid=r.read(12); extra=f" q=UNIQUE[{sid}]={UNIQ.get(sid,{}).get('index','?')}"
    if runeword: extra+=f" RW=({r.read(12)},{r.read(4)})"
    if personalized:
        nm=""
        while True:
            c=r.read(8)
            if c==0: break
            nm+=chr(c)
        extra+=f" nom={nm!r}"
    if code=="tbk" or code=="ibk": r.read(5)
    realm=r.read(1)
    if realm: r.read(96); extra+=" REALM(96)"
    td=""
    if code in ARMOR:
        dv=r.read(11)-ISC.get(31,{}).get('sa',0)
        mx=r.read(8)-ISC.get(72,{}).get('sa',0)
        du=(r.read(8)-ISC.get(73,{}).get('sa',0)) if mx>0 else 0
        td=f" def={dv} dur={du}/{mx}"
    elif code in WEAP:
        mx=r.read(8)-ISC.get(72,{}).get('sa',0)
        du=(r.read(8)-ISC.get(73,{}).get('sa',0)) if mx>0 else 0
        td=f" dur={du}/{mx}"
    if code in STACKABLE:
        qty=r.read(9); td+=f" qty={qty}"
    if socketed: td+=f" sockN={r.read(4)}"
    mask=0
    if q==5:
        mask=r.read(5); td+=f" setmask={mask:05b}"
    mods=read_mods(r)
    ms=" ".join(f"{v:+d} {st}(id{i})" for i,v,st,w in mods if v is not None)
    if any(v is None for i,v,st,w in mods): ms+=" !!WIDTH?"
    if q==5:
        prop=(1|mask)
        for bit in (1,2,4,8,16,32,64):
            if prop & bit and bit != 1:
                l2=read_mods(r)
                extra+=" SETLIST["+" ".join(f"{v:+d} {st}" for i,v,st,w in l2 if v is not None)+"]"
    print(f"{ind}{code:5} ilvl={ilvl:2} fid={fid:#010x} q={QN.get(q,q):6}{' ETH' if ethereal else ''}{' sock' if socketed else ''}{td}{extra} mods=[{ms}] {desc} @{start//8}.{start%8}")
    r.align()

def parse_list(r, label):
    jm=r.read(8),r.read(8)
    assert bytes(jm)==b"JM", f"{label}: JM attendu, got {bytes(jm)}"
    cnt=r.read(16)
    print(f"--- {label}: {cnt} items @byte {r.p//8}")
    for i in range(cnt):
        parse_item(r)
    return cnt

r=R(848*8)
n=parse_list(r,"character")
p=r.p//8
print("fin char:", p)
if data[p:p+2]==b"JM": parse_list(r,"corpse")
p=r.p//8
if data[p:p+2]==b"jf":
    r.read(16); parse_list(r,"merc"); 
p=r.p//8
if data[p:p+2]==b"kf":
    r.read(16); g=r.read(8); print("golem flag:", g)
    if g: parse_list(r,"golem")
p=r.p//8
print("FIN:", p, "/ taille", len(data), "OK" if p==len(data) else f"RESTE: {data[p:].hex(' ')}")
