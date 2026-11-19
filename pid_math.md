H(s) is the transfer function for position.

H(s) = a/(bs^2+s)

P(s) = H(s)I(s)
I(s) is input
P(s) is the position function

I want the transfer function for
V(s) the speed function

v(t) = d/dt p(t) (by defintion)
V(s) = SP(s) - p(0) (Taking the leplace of both sides) 
let p(0) = 0
V(s) = S H(s) I(S)
V(s) = (s * a/(bs^2+s)) * I(S)
V(s) = (s * a/(s(bs+1)) * I(S)
V(s) = a/(bs+1) * I(S)

let G(s) = a/(bs+1)
V(s) = G(s)I(s)

Using a feed back loop of the form

-> - -> G(s) -> K(s) -> 
   ^              |
   ----------------

T(s) = K(s)G(s) / (1+K(S)G(S))

K(s) = c + d / s + fs

T(s) = (c+d/s+fs)*(a/(bs+1))/(1+(c+d/s+fs)*(a/(bs+1)))
Let L = (c+d/s+fs)*(a/(bs+1))
T(s) = L/(1+L)
L = (c+d/s+fs)*(a/(bs+1))
L = (a(cs + d + fs^2))/(s(bs + 1))
let A = (a(cs + d + fs^2))
let B = (s(bs + 1))
T(s) = (A/B)/(1+A/B)
T(s) = A / (B + A)
T(s) = (a(c*s + d + fs^2))/((a(c*s + d + fs^2)) + (s(b*s + 1)))
T(s) = (a(c*s + d + fs^2))/((a(c*s + d + fs^2)) + (s(b*s + 1)))
let C = ((a(c*s + d + fs^2)) + (s(b*s + 1)))
C = acs + ad + afs^2 + bs^2 + s
C = afs^2 + bs^2 acs + s + ad
C = (af+b)s^2 + (ac+1)s + ad
T(s)= (a(c*s + d + fs^2)) / ((af+b)s^2 + (ac+1)s + ad)
T(s)= (afs^2 + acs + ad) / ((af+b)s^2 + (ac+1)s + ad)
T(s)= (afs^2/(af+b) + acs/(af+b) + ad/(af+b)) / (s^2 + (ac+1)/(af+b)s + ad/(af+b))
D = (afs^2/(af+b) + acs/(af+b) + ad/(af+b))
N = (s^2 + (ac+1)/(af+b)s + ad/(af+b))
T(s)= D/N

## restriction 1
If 0 roots are in the left half of the plane then 
D = (afs^2/(af+b) + acs/(af+b) + ad/(af+b)) 
get zero roots of this by D = 0
0 = afs^2/(af+b) + acs/(af+b) + ad/(af+b)
0 = fs^2 + cs + d (devide both sides by a/(af+b))
0 = fs^2 + cs + d
s_r is a root of D
s_r (real component) < 0 (This has to be true)

when f = 0
then
0 = cs_r + d
-cs_r = d
s_r = -d/c
as d and c are both real
-d/c < 0
d/c > 0
(d and c must have same sign and c can't be zero)

else when f =/= 0

Use Routh-Hurwitz stability criterion

Table:
| :--- | :---: | :---: |
|f|d|0|
|c|0|0|
|d|0|0|
|0|0|0|

It follows that 
this is the case if and only if 
c, d, f all share a sign.

## Optimization values

N = (s^2 + (ac+1)/(af+b)s + ad/(af+b))

T(s) = D/N
T(S) = D/(s^2 + (ac+1)/(af+b)s + ad/(af+b))

When the roots of D are in the left of the plane this is aproxamtly.
T(s) ~= (ad/(af+b))/(s^2 + (ac+1)/(af+b)s + ad/(af+b))

let P(s) = (ad/(af+b))/(s^2 + (ac+1)/(af+b)s + ad/(af+b))

put p(s) in the form
kw^2/(s^2+2zw+w^2)

w^2 = ad/(afb)
w = sqrt(ad/(af+b))
2zw = (ac+1)/(af+b)
z = (ac+1)/(af+b) / (2w)
z = (ac+1)/(af+b) / sqrt(ad/(af+b))

kw^2 = (ad/(af+b))
k = (ad/(af+b))/w^2
k = (ad/(af+b))/(ad/(af+b))
k = 1

overshot:
Mp =  exp(-pi*z/sqrt(1-z^2))
rise time
Tr = (pi - tan-1(sqrt(1-z^2)/z))/(w(sqrt(1-z^2)))

lets set some max Mp
Mp >= exp(-pi*z/sqrt(1-z^2))
Assume z is in range 0 to 1
Use Reduce[M >= Exp[-Pi z/Sqrt[1 - z^2]] && 0 < z < 1, z, Reals] in wolframplha
|ln(Mp)|/sqrt(ln(Mp)^2+pi^2) <= z < 1

Let set some max Tr 
Tr = (pi - tan-1(sqrt(1-z^2)/z))/(w(sqrt(1-z^2)))
Wolframlpha Solve T >= (pi - arctan(sqrt(1-z^2)/z))/(w*(sqrt(1-z^2))) for w, T>0, w>0, 0<z<1
w >= pi - tan-1(sqrt(1-z^2)/z)/(Tr*sqrt(1-z^2))

There is no way to solve for z so will just have to check soltions.

----

I don't want a biproper transfer fuinction so

f = 0
witch coluaps D to
D = acs/b + ad/b
and N to
N = (s^2 + ((ac+1)/b)s + ad/b)

From resitriction one I now know
c and d must have the same sign.

w = sqrt(ad/b)
z = ((ac+1)/b / sqrt(ad/b))

you can solve for c, d
Use solve {w = sqrt(a*d/b), z = ((a*c+1)/b)/sqrt(a*d/b)} for {c,d} wolframaplha
c = (b w z - 1)/a
d = (b w^2)/a

c and d have same sign 
cd >= 0
((b w z - 1)/a)((b w^2)/a) >= 0
((b w z - 1))((b w^2))a^-2 >= 0
given a is real
(b w z - 1)(b w^2) >= 0
b^2(w z - 1/b)(w^2) >= 0
given b is real
(wz - 1/b)w^2 >= 0
w^3*z - w^2/b >= 0
w^3*z >= w^2/b
given w is real
wz >= 1/b


In terms of w, z, a, b

wz >= 1/b
w >= pi - tan-1(sqrt(1-z^2)/z)/(Tr*sqrt(1-z^2))
|ln(Mp)|/sqrt(ln(Mp)^2+pi^2) <= z < 1

algorth, do basic grid search, where you 
set go from z_min to z_max at descreate steps and attempt 
solve for w at each point using  pi - tan-1(sqrt(1-z^2)/z)/(Tr*sqrt(1-z^2))
check that wz >= 1/b if it dose use that point.

It should then output 
c = (b w z - 1)/a
d = (b w^2)/a
