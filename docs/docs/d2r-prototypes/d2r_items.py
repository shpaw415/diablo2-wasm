import struct, sys
data = open('/tmp/opencode/Javazon.d2s','rb').read()
START = 848*8

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
    def ascii(s,n): return bytes(s.read(8) for _ in range(n)).decode('ascii','replace')
    def align(s): s.p=(s.p+7)&~7

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
STACKABLE={"tbk","ibk","key","tsc","isc","vps","yps","ww1","ww2","ww3","aqv","cqv","xqv",
 "tkf","tax","bkf","bal","9tk","9ta","9bk","8lx","7bk","7tk","9ts","7ts",
 "jav","pil","ssp","glv","tsp","9ja","9pi","9ts","7ja","7pi","9ja","7s7","7gl","7ts",
 "am5","ama","amf","hpo","hp1","hp2","hp3","hp4","hp5","mp1","mp2","mp3","mp4","mp5",
 "rps","bps","rvs","rvl","gps","ops","gpm","opl","gpl","vps","wms","elx"}

N=[0]
def parse_item(r, depth=0):
    ind="  "*depth
    start=r.p
    flags=[r.read(1) for _ in range(32)]
    identified, socketed, ear, simple, ethereal = flags[4],flags[11],flags[16],flags[21],flags[22]
    personalized, runeword = flags[24],flags[26]
    ver=r.read(3); parent=r.read(3); equip=r.read(4); x=r.read(4); y=r.read(4); stored=r.read(3)
    N[0]+=1
    desc=f"v{ver} par={parent} eq={equip} x={x} y={y} st={stored}"
    if ear:
        cls=r.read(3); lvl=r.read(7)
        nm=""
        while True:
            c=r.read(8)
            if c==0: break
            nm+=chr(c)
        print(f"{ind}EAR class={cls} lvl={lvl} name={nm!r} {desc}")
        r.align(); return
    code=r.ascii(4).rstrip('\x00 ')
    sockcnt = r.read(1) if simple else r.read(3)
    extra=""
    if not simple:
        fid=r.read(32); ilvl=r.read(7); q=r.read(4)
        qn={1:"low",2:"normal",3:"sup",4:"magic",5:"set",6:"rare",7:"unique",8:"crafted"}.get(q,q)
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
            extra=f" q=rare names=({n1},{n2}) aff={aff}"
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
        if realm: extra+=f" REALM96={r.read(96)}" if False else " REALM_DATA(96bits)"
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
        if (code in STACKABLE) or (code in MISC and code in STACKABLE):
            qty=r.read(9); td+=f" qty={qty}"
        if socketed:
            td+=f" sockN={r.read(4)}"
        if q==5:
            mask=r.read(5); td+=f" setmask={mask}"
        else:
            mask=0
        mods=[]
        while True:
            sid=r.read(9)
            if sid==0x1ff: break
            w=mod_width(sid)
            if w is None:
                mods.append((sid,"??")); break
            mods.append((sid, r.reads(w), ISC.get(sid,{}).get('stat','?'), w))
        ms=" ".join(f"{v:+d} {st}(id{i},w{ww})" for i,v,st,ww in mods if len((i,v,st,ww))==4)
        prop=1|mask
        lists=0
        for i in (1,2,4,8,16,32,64):
            if prop & i: lists+=1
        for _ in range(lists-1 if q==5 else 0):
            l2=[]
            while True:
                sid=r.read(9)
                if sid==0x1ff: break
                w=mod_width(sid)
                if w is None: break
                l2.append((sid, r.reads(w), ISC.get(sid,{}).get('stat','?'), w))
            extra+=f" SETLIST[{'; '.join(f'{v:+d} {st}' for i,v,st,ww in l2 if len((i,v,st,ww))==4)}]"
        print(f"{ind}{code:5} ilvl={ilvl:2} fid={fid:#010x}{td}{extra} mods=[{ms}] {desc} @{start//8}(bit{start%8})")
    else:
        print(f"{ind}SIMPLE {code:5} sock1bit={sockcnt} {desc} @{start//8}(bit{start%8})")
    r.align()

r=R(START)
jm=r.ascii(2); cnt=r.read(16)
print(f"JM count={cnt} @{START//8}")
assert jm=="JM"
for i in range(cnt):
    try:
        parse_item(r)
    except Exception as e:
        import traceback; traceback.print_exc()
        print(f"!! STOP item {i} @bit {r.p} byte {r.p//8}"); break
p=r.p//8
print("fin:", p, "attendu 2829", "reste:", data[p:p+12].hex(' '))
