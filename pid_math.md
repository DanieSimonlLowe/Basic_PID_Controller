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
O(s)=T(s)I(s)

Let the input I(s) be 1/s (non-shifted unit step function)

O(s)=(1/s)*(c+d/s+fs)*(a/(bs+1))/(1+(c+d/s+fs)*(a/(bs+1)))
O(s)=(-b s - 1)/(a c s + a d + a f s^2 + b s^2 + s) + 1/s
o(t)=1 - Exp[-(1+a c) t/(2B)] / B * ( b Cosh[S t/(2B)] + (b(1 - a c) + 2 a f)/S * Sinh[S t/(2B)] )
is not in the correct form

Let
K(s) = c + fs
O(s)=(1/s)*(c+fs)*(a/(bs+1))/(1+(c+fs)*(a/(bs+1)))
o(t)=a (-(c e^(-(a c t + t)/(a f + b)))/(a c + 1) + (f e^(-(a c t + t)/(a f + b)))/(a f + b) + c/(a c + 1))
o(t)=(a c)/(a c + 1) - (a (b c - f) exp(-(a c t)/(a f + b) - t/(a f + b)))/((a c + 1) (a f + b))

Let
K(s) = c + d/s
O(s)=(1/s)*(c+d/s)*(a/(bs+1))/(1+(c+d/s)*(a/(bs+1)))
o(t) = a (a^(-1) - (-E^((-1/(2 b) - (a c)/(2 b) - Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + a c E^((-1/(2 b) - (a c)/(2 b) - Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d] E^((-1/(2 b) - (a c)/(2 b) - Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + E^((-1/(2 b) - (a c)/(2 b) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) - a c E^((-1/(2 b) - (a c)/(2 b) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d] E^((-1/(2 b) - (a c)/(2 b) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t))/(2 a Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]))

let S = Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]
o(t) = a (a^(-1) - (-E^((-1/(2 b) - (a c)/(2 b) - Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + a c E^((-1/(2 b) - (a c)/(2 b) - Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d] E^((-1/(2 b) - (a c)/(2 b) - Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + E^((-1/(2 b) - (a c)/(2 b) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) - a c E^((-1/(2 b) - (a c)/(2 b) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d] E^((-1/(2 b) - (a c)/(2 b) + Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]/(2 b)) t))/(2 a Sqrt[1 + 2 a c + a^2 c^2 - 4 a b d]))