data = open('/tmp/opencode/Javazon.d2s','rb').read()
def bits(start, n):
    out=[]
    for i in range(n):
        b=data[(start+i)//8]
        out.append(str((b>>((start+i)%8))&1))
    return ''.join(out)
HUFF = {' ':"10",'0':"11111011",'1':"1111100",'2':"001100",'3':"1101101",'4':"11111010",
'5':"00010110",'6':"1101111",'7':"01111",'8':"000100",'9':"01110",'a':"11110",'b':"0101",
'c':"01000",'d':"110001",'e':"110000",'f':"010011",'g':"11010",'h':"00011",'i':"1111110",
'j':"000101110",'k':"010010",'l':"11101",'m':"01101",'n':"001101",'o':"1111111",'p':"11001",
'q':"11011001",'r':"11100",'s':"0010",'t':"01100",'u':"00001",'v':"1101110",'w':"00000",
'x':"00111",'y':"0001010",'z':"11011000"}
import sys
# item @892 (amu) — dump flags..code
for name, start in [("amu@892",892*8), ("rin@921",921*8), ("ww@949",949*8)]:
    bs = bits(start, 130)
    print(f"== {name}: {bs}")
    # flags
    print("   flags:", bs[:32], "identified=",bs[4], "socketed=",bs[11], "ear=",bs[16], "simple=",bs[21], "eth=",bs[22], "perso=",bs[24], "rw=",bs[26])
    p=32
    ver=bs[p:p+3]; p+=3; par=bs[p:p+3]; p+=3; eq=bs[p:p+4]; p+=4; x=bs[p:p+4]; p+=4; y=bs[p:p+4]; p+=4; st=bs[p:p+3]; p+=3
    print(f"   ver={int(ver[::-1],2)} par={int(par[::-1],2)} eq={int(eq[::-1],2)} x={int(x[::-1],2)} y={int(y[::-1],2)} st={int(st[::-1],2)}")
    # huffman decode
    code=""; consumed=0
    while consumed < 40:
        code += bs[p+consumed]
        consumed += 1
        if code in HUFF:
            print(f"   code char {HUFF[code]!r} <- {code} ({consumed} bits cumulés)")
            if HUFF[code]==' ': break
            code=""
    p += consumed
    print("   après code, bit pos relative:", p)
    print("   60 bits suivants:", bs[p:p+60])
