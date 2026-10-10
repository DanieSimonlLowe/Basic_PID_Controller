The input from the random input can be thourt of as a peacwise function

f(t) = {
x < t[0], 0
t[0] <= x < t[1], w[0]
t[1] <= x < t[2], w[1]
  ...
t[n-1] <= x < t[n], w[n-1]
t[n] <= x, w[n]
}


Thie can be considered a sum of weighted offset unit step functions (u(t))

let d[0] = w[i]
let d[i] = w[i] - w[i-1]

i(t) = sum d[i]*u(t-t[i]) [from i = 0 to i = n]

By the fact the linearity of the Leplace transform

L[i(t)] = sum d[i] * L(u(t-t[i]) [from i = 0 to i = n]
I(s) = sum d[i] * exp(-t[i] * s)/s [from i = 0 to i = n]


Assume the motor has a transfer function
H(s) = a/(bs^2+s)

Then the output
O(s) = I(s)H(s)
O(s) = (sum d[i] * exp(-t[i] * s)/s [from i = 0 to i = n]) * H(s)
O(s) = (sum d[i] * exp(-t[i] * s)/s * H(s) [from i = 0 to i = n])
let O[i](s) = d[i] * exp(-t[i] * s)/s * H(s)
O(s) = (sum O[i](s) [from i = 0 to i = n])

O[i](s) = d[i] * exp(-t[i] * s)/s * a/(bs^2+s)
O[i](s) = a * d[i] * exp(-t[i] * s) / (s*(bs^2+s))
From InverseLaplaceTransform[a * d * exp(-p * s) / (s * (b*s^2 + s)), s, t] in wolfram alpha
o[i](t) = a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])

Given the linearity of the Leplace transform
o(t) = (sum o[i](t) [from i = 0 to i = n])



Let the y(t) be the measured output
so the loss l would be
l = sum (y(t) - o(t))^2 [for all measured t]
l = sum (y(t) - (sum o[i](t) [from i = 0 to i = n]))^2 [for all measured t]
let l[t] = (y(t) - (sum o[i](t) [from i = 0 to i = n]))^2

l[t] = (y(t) - (a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n])^2
(so you can calulate loss for a point and therefor total loss)

By the Addition Rule of differentiation
dl/da= sum dl[t]/da [for all measured t]
dl/db= sum dl[t]/db [for all measured t]

dl[t]/da = d/da ((y(t) - SUM (a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n])^2)
dl[t]/da = d/da ((y(t) - a * SUM (d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n])^2)
Let S = (d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n]
dl[t]/da = d/da ((y(t) - a * S)^2
S behaves as a constent with respect to a so 
dl[t]/da = -2*S*(y - a*S) 
(The direviate of loss with respect to a at a point)


dl[t]/db = d/db ((y(t) - SUM (a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n])^2)
let f(b) = SUM (a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n]
dl[t]/db = d/db (y(t) - f(b))^2
dl[t]/db = -2f'(b)*(y-f(b))
f(b) = SUM (a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n]
let f[i](b = a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])
f(b) = SUM (f[i](b)) [from i = 0 to i = n]
By the Addition Rule of differentiation
f'(b) = SUM (f'[i](b)) [from i = 0 to i = n]
f'[i](b) = d/db a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])
By wolfram aplha with d/db a * d * (b * (exp((T-t)/b)-1) - T + t) * u(t-T)  gives (a d e^(T/b - t/b) (b + t - T) u(t - T))/b - a d u(t - T)
f'[i](b) = ((a*d[i]*exp((t[i]-t)/b)*(b+t-t[i]))/b - a*d[i]) * u[t-t[i]]

dl[t]/db = -2f'(b)*(y-f(b))

dl[t]/db = -2*a*(SUM f'[i](b) [from i = 0 to i = n])*(y-a*(SUM f[i](b) [from i = 0 to i = n]))
f'[i](b) = ((d[i]*exp((t[i]-t)/b)*(b+t-t[i]))/b - d[i]) * u[t-t[i]]
f[i](b) =  d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])
(using this we can calulate dl[t]/db easly


Loss at a point (sum to get total loss)
l[t] = (y(t) - (a * d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n])^2

d/da Loss at a point
S = (d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])) [from i = 0 to i = n]
l[t] = (y(t) - aS)^2

So you can estimate minimal loss for a as
a ~= (sum (S[t]*y(t)))/(sum (S[i]*S[i]))
This dose not take into account b


d/db Loss at point
dl[t]/db = -2*a(SUM f'[i](b) [from i = 0 to i = n])*a*(y-(SUM f[i](b) [from i = 0 to i = n]))
f'[i](b) = ((d[i]*exp((t[i]-t)/b)*(b+t-t[i]))/b - d[i]) * u[t-t[i]]
f[i](b) = d[i] * (b * (exp((t[i]-t)/b)-1) - t[i] + t) * u(t-t[i])


l = sum (y(t) - af(b))^2
let a*(b) be the optmal a for b
let dl*/db be dl/db where a is a function of b
dl*/db = dl/db + (dl/da)(da*/db) by cain rule.
dl/da = 0 as a is optomal value
dl*/db = dl/db


TODO find something on a minimization method.
Algorth to calulate b

start_b = b_min-step
start_loss = calulate the loss at start_b
mid_loss = inf
mid_b = 0


min_loss = inf
min_loss_b = 0
for each b_guess in range(b_min, b_max, step_size):
    loss = calulate the loss at b_guess
    if (loss < start_loss && loss < mid_loss)
        start_loss = mid_loss
        start_b = mid_b
        mid_loss = loss
        mid_b = b
    if (loss > mid_loss)
        optimize(start_b, mid_b, b, l, dl/db)
        
        

func optimize(l, m, r, f, f'):
    l < m < r
    f(m) < l
    f(m) < r
    use brents method https://www2.boulder.swri.edu/~layoung/idl/layoung/math/dbrent.pro
    https://docs.rs/crate/basin/1.3.0/source/src/solver/brent_derivative.rs
    https://waxworksmath.com/Software/Code/NRecipes2Matlab/nr_dbrent.m

