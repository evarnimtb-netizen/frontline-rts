import math, itertools
R={'M61':(75,0,[88,81,73]),'M62':(76,0,[125,116,106]),'M93':(76,1,[239,208,175]),'M82':(90,0,[164,152,137]),
'L70':(75,0,[185,168,149]),'L70R':(75,1,[265,234,199]),'88':(88,0,[162,151,138]),'88R':(88,1,[219,200,179]),'L48':(75,0,[135,123,109]),'L48R':(75,1,[176,154,130])}
def pen(p,r): return p[0]+(p[1]-p[0])*(r-100)/400 if r<=500 else p[1]+(p[2]-p[1])*(r-500)/500
def chance(need):  # P(1+v >= need), v triangular [-.07,.07]
  x=(need-1)/0.07
  if x<=-1: return 1.0
  if x>=1: return 0.0
  return 1-((x+1)**2/2) if x<0 else (1-x)**2/2
def ch(rd,t,s,r,lo,hi,on=True):
  cal,apcr,p=R[rd]; c=math.cos(math.radians(s))
  w=min(1,max(0,(t/cal-lo)/(hi-lo))) if on else 0
  ce=c*(1-w+w*(c if apcr else math.sqrt(c)))
  return chance(t/(pen(p,r)*ce))
checks=[('a M62 glacis 100 =0',lambda lo,hi:ch('M62',80,55,100,lo,hi)<=0.0),
('b M93 glacis 100>=25',lambda lo,hi:ch('M93',80,55,100,lo,hi)>=0.25),
('b M93 glacis 400<=15',lambda lo,hi:ch('M93',80,55,400,lo,hi)<=0.15),
('c M82 glacis 100<=20',lambda lo,hi:ch('M82',80,55,100,lo,hi)<=0.20),
('e M62 nose 350<=30',lambda lo,hi:ch('M62',60,55,350,lo,hi)<=0.30),
('f L70 sherman 1000>=90',lambda lo,hi:ch('L70',63.5,47,1000,lo,hi)>=0.9),
('g 88 jumbo 300<=20',lambda lo,hi:ch('88',101.6,47,300,lo,hi)<=0.2)]
for lo,hi in itertools.product([0.25,0.3,0.4,0.5],[0.75,0.8,0.9,1.0]):
  if lo>=hi: continue
  res=[n for n,f in checks if not f(lo,hi)]
  print(lo,hi,'FAIL:',res)
print()
lo,hi=0.4,0.9
def chy(rd,t,s,r,yaw,on):
  cal,apcr,p=R[rd]; c=math.cos(math.radians(s))*math.cos(math.radians(yaw))
  w=min(1,max(0,(t/cal-lo)/(hi-lo))) if on else 0
  ce=c*(1-w+w*(c if apcr else math.sqrt(c)))
  return chance(t/(pen(p,r)*ce))
rows=[('M62','Panther glacis',80,55),('M93','Panther glacis',80,55),('M82','Panther glacis',80,55),('88','Panther glacis',80,55),('L70R','Panther glacis',80,55),
('M62','Panther nose',60,55),('M93','Panther nose',60,55),('M82','Panther nose',60,55),
('M62','Panther turret 100@12',100,12),('M82','Panther turret 100@12',100,12),
('L70','Jumbo glacis',101.6,47),('L70R','Jumbo glacis',101.6,47),('88','Jumbo glacis',101.6,47),('88R','Jumbo glacis',101.6,47),('L48R','Jumbo glacis',101.6,47),
('L70','Sherman glacis',63.5,47),('L48','Sherman glacis',63.5,47),('88','Sherman glacis',63.5,47),
('M62','Tiger front',100,8),('M82','Tiger front',100,8),('M93','Tiger front',100,8)]
print('round | plate | yaw | 100 | 350 | 500 | 1000   (base -> blend, %)')
for rd,n,t,s in rows:
  for yaw in (0,30):
    cells=[f"{100*chy(rd,t,s,r,yaw,False):.0f}->{100*chy(rd,t,s,r,yaw,True):.0f}" for r in (100,350,500,1000)]
    print(f"| {rd} | {n} {t}@{s} | {yaw} | "+' | '.join(cells)+' |')
