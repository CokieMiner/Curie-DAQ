# Statistics in Curie-DAQ

How every number the app shows or writes to a CSV is computed. The formulas here are the ones in
the code: `crates/curie-core/src/analysis/` (fits), `crates/curie-core/src/stats/` (moments and
histograms), `crates/curie-ui/src/export.rs` (CSV columns) and `crates/curie-ui/src/pages/` (what
each page displays).

Notation: a run is a series of $n$ gates; gate $i$ lasts $\Delta t_i$ (usually the same $\Delta t$
for all) and counts $N_i$ pulses. $\bar N$ is the mean count per gate.

---

## 1. One gate: count, rate and uncertainty

Radioactive decay is a Poisson process: the number of pulses in a gate has variance equal to its
mean, and the measured count is the best estimate of that mean. So

$$
R = \frac{N}{\Delta t}, \qquad \sigma_R = \frac{\sqrt{\max(N, 1)}}{\Delta t}.
$$

A gate with **zero counts** gets $\sigma = 1/\Delta t$ instead of $0$: $\sqrt{0} = 0$ would claim
an exact measurement, and a weighted fit would give that gate infinite weight. This is the usual
"±1 count" convention for empty bins.

The **time** of a gate is its centre, measured from the start of the run: the gate start plus
$\Delta t/2$. With the Cobra the gate is timed by the interface (µs resolution) and the gap between
gates (reading the count, arming the next gate) is not counted, and because each gate is placed at
its real start time it does not distort a decay curve. **PAUSE** throws away the gate in progress,
so every recorded gate has its full length; the run clock keeps running through the pause.

### Dead-time correction

After each pulse a Geiger-Müller tube is blind for its dead time $\tau$. With the non-paralysable
model the true rate is

$$
R_\text{true} = \frac{R}{1 - R\,\tau}.
$$

$\tau$ comes from the detector chosen in 4:CONFIG: 100 µs for the PHYWE 09005.00 (typical value),
20 µs for the LND 7311 (datasheet minimum, so the correction is a lower bound), or a value
measured in the lab with CUSTOM. The correction is written to the CSV column
`rate_deadtime_corrected_cps` (`NaN` when $R\tau \ge 1$, where the model breaks down) and is
applied by the decay fit (§6). The plots, live metrics and distribution fits use the raw counts:
dead time changes the shape of the count distribution, so correcting the counts would not make
them Poisson.

At 100 µs the correction is about 1 % at 100 counts/s and 11 % at 1000 counts/s.

---

## 2. Live metrics (1:ACQUIRE)

Computed from the raw counts of all gates of the current run:

| shown as | formula | meaning |
|---|---|---|
| MEAN COUNTS PER GATE | $\bar N \pm s/\sqrt n$ | sample mean and its standard error |
| rate | $\bar N/\Delta t \pm s/(\sqrt n\,\Delta t)$ | mean count rate |
| VARIANCE | $s^2 = \frac{1}{n-1}\sum (N_i - \bar N)^2$ | unbiased sample variance |
| FANO | $F = s^2/\bar N$ | 1 for a Poisson process |

The standard error here comes from the **observed spread** of the gates, not from $\sqrt{\bar N}$,
so it stays honest if the counts are not Poisson. The Fano factor is the quick check: close to 1
for a long-lived source, well above 1 when the rate changes during the run (a decaying source, a
drifting setup), below 1 when pulses are lost (heavy dead time).

The mean and variance are updated one gate at a time with Welford's algorithm, which stays exact
even when the mean is much larger than the spread (e.g. $10^8 \pm 1$ counts).

---

## 3. Histogram of counts per gate

Counts are integers, so every bin covers a whole number of integer values: bin $j$ holds the
counts $a_j \dots b_j$, with width $w = b_j - a_j + 1$, and the first bin starts at the largest
multiple of $w$ not above the smallest count. No count is ever clamped into an edge bin.

* **FIXED**: $w$ is the width you type (1 = one bin per count value).
* **AUTO** (default): $w = \lceil (N_\text{max} - N_\text{min} + 1) / B \rceil$ for about $B$ bins
  (30 by default).

If the range would need more than 4096 bins, the width is increased instead.

---

## 4. Poisson fit

The maximum-likelihood estimate of the Poisson mean is the sample mean of the **raw counts** (not
of histogram bin centres):

$$
\hat\lambda = \bar N, \qquad \sigma_{\hat\lambda} = \sqrt{\hat\lambda / n}.
$$

The panel also shows the sample variance $s^2$ and the Fano factor $s^2/\hat\lambda$; for Poisson
data both $s^2 \approx \hat\lambda$ and $F \approx 1$.

The **expected number of gates** in bin $j$ sums the exact Poisson probabilities over every integer
in the bin,

$$
E_j = n \sum_{k=a_j}^{b_j} \frac{\hat\lambda^k e^{-\hat\lambda}}{k!},
$$

with everything below the first bin added to the first bin and everything above the last bin added
to the last, so the $E_j$ add up to $n$. These are the diamonds drawn over the histogram.

---

## 5. Gaussian fit

$$
\hat\mu = \bar N, \qquad \hat\sigma = s, \qquad
\sigma_{\hat\mu} = \frac{s}{\sqrt n}, \qquad
\sigma_{\hat\sigma} \approx \frac{s}{\sqrt{2(n-1)}},
$$

$$
\text{FWHM} = 2\sqrt{2\ln 2}\;\hat\sigma \approx 2.3548\,\hat\sigma .
$$

$\hat\sigma$ uses $n-1$ (unbiased variance), not the maximum-likelihood $n$; the difference
matters only for very short runs. The panel also shows $\sqrt{\hat\mu}$: for Poisson counts the
Gaussian width should be close to it.

Counts are integers, so the expected frequency of a bin integrates the normal density over the bin
widened by half a count on each side (continuity correction), with the tails folded into the end
bins:

$$
E_j = n\left[\Phi\!\left(\frac{b_j + \tfrac12 - \hat\mu}{\hat\sigma}\right)
           - \Phi\!\left(\frac{a_j - \tfrac12 - \hat\mu}{\hat\sigma}\right)\right],
$$

where $\Phi$ is the standard normal cumulative distribution ($\Phi = 0$ below the first bin and
$1$ above the last).

---

## 6. Goodness of fit (Pearson χ²)

Pearson's test needs enough expected events per bin, so neighbouring bins are merged first: going
up from the lowest bin, bins are added to a group until the group expects at least 5 gates; a
remainder at the top that expects fewer than 5 joins the last group. Then

$$
\chi^2 = \sum_\text{groups} \frac{(O_g - E_g)^2}{E_g}, \qquad
\text{ndf} = G - 1 - p, \qquad \chi^2_\text{red} = \chi^2 / \text{ndf},
$$

with $G$ groups, $O_g$ observed and $E_g$ expected gates, and $p$ fitted parameters ($p = 1$ for
Poisson, $p = 2$ for Gaussian). A good fit has $\chi^2/\text{ndf}$ near 1; the panel also reports
how many groups were formed. When ndf is 0 there are too few gates for the test and the reduced
χ² is shown as "—".

---

## 7. Decay fit and half-life (3:ANALYSIS → EXP)

### Model

$$
\mu_i = \Delta t_i \left( A\, e^{-\lambda (t_i - t_0)} + B \right),
$$

the expected counts of gate $i$ centred at $t_i$. $t_0$ is the first gate in the fit window,
$A$ the net rate at $t_0$, $\lambda$ the decay constant and $B$ the background rate. The window is
set with SKIP FIRST (gates) and START AT (time). $B$ is either fitted, or **fixed** to a background
("zero effect") measured separately, which is better when the run does not reach the background
level.

Before fitting, each count is corrected for dead time,
$N_i' = N_i / (1 - N_i \tau / \Delta t_i)$; gates where less than 10 % of the time is live are
dropped.

### Why Poisson maximum likelihood

The classic method fits a straight line to $\ln(R_i - B)$. It has to drop or distort every gate
where $R_i - B \le 0$, which happens all the time once the source has decayed to the background,
and that biases the half-life upward (on 15-minute simulated Pa-234m runs it gave about 96 s
instead of 70.2 s). Curie-DAQ instead maximises the Poisson likelihood of the counts themselves,
which uses every gate, including those at the background level. This is the same as minimising
the Poisson deviance

$$
D = 2 \sum_i \left[ \mu_i - N_i + N_i \ln \frac{N_i}{\mu_i} \right].
$$

On the same simulated runs the fit gives $T_{1/2} = 70.2$ s on average.

### Minimisation

Levenberg–Marquardt with Fisher scoring: each step solves the Fisher information matrix

$$
I_{jk} = \sum_i \frac{1}{\mu_i}\,\frac{\partial \mu_i}{\partial \theta_j}\,
         \frac{\partial \mu_i}{\partial \theta_k},
\qquad \theta = (A, \lambda, B),
$$

with its diagonal enlarged by a damping factor that shrinks after a successful step and grows
after a failed one. Steps must keep $A > 0$, $\lambda > 0$, $B \ge 0$. The fit stops when the
deviance improves by less than $10^{-12}(1 + D)$, or when no step lowers it; after 500 iterations
it stops and the panel warns that it did not fully converge.

Starting values: $B$ from the mean rate of the last fifth of the gates (or the fixed value), then
a weighted straight line through $\ln(R_i - B)$, using only the gates more than $2\sigma$ above
$B$, for $A$ and $\lambda$.

### Results and uncertainties

The uncertainties are the square roots of the diagonal of $I^{-1}$ at the minimum (the usual
large-sample approximation for maximum-likelihood estimates). From $\lambda$:

$$
T_{1/2} = \frac{\ln 2}{\lambda}, \qquad
\sigma_{T_{1/2}} = \frac{\ln 2\;\sigma_\lambda}{\lambda^2}.
$$

With a fixed background, $B$ has no uncertainty and only $A$ and $\lambda$ are fitted. The panel
also shows

$$
\chi^2 = \sum_i \frac{(N_i' - \mu_i)^2}{\mu_i}, \qquad \text{ndf} = n - p
$$

($p = 3$, or 2 with $B$ fixed), and the number of gates used. The LOG view plots
$\ln(R_i - B)$ for gates above the background; it is only a display, the fit always uses all
gates in the window.

---

## 8. Sweep table (2:SWEEP)

Each cell is the rate of one state at one position, $R \pm \sigma_R$ as in §1. If a position and
state are measured again, the table shows the latest measurement; the autosave CSV keeps every one.

With two states the table adds their difference and ratio. The two counts are independent, so
their uncertainties add in quadrature:

$$
S_1 - S_2 \;\pm\; \sqrt{\sigma_1^2 + \sigma_2^2},
$$

$$
r = \frac{S_2}{S_1} \;\pm\; \frac{\sqrt{\sigma_2^2 + r^2 \sigma_1^2}}{S_1}.
$$

The ratio's uncertainty is the standard propagation $r\sqrt{(\sigma_1/S_1)^2 + (\sigma_2/S_2)^2}$,
written so that it stays finite when $S_2 = 0$; for equal gates and non-zero counts it equals
$r\sqrt{1/N_1 + 1/N_2}$. The table shows raw rates; the CSV also has the dead-time corrected rate.

---

## 9. Simulator

The simulator draws one count per gate from a Poisson distribution with mean
$\bar\mu = (R_\text{source} + R_\text{background})\,\Delta t$:

* $\bar\mu \le 60$: exact Poisson sampling (Knuth's multiplication method).
* $\bar\mu > 60$: a normal value with mean $\bar\mu$ and variance $\bar\mu$, rounded to an integer.
  This approximation ignores the small skewness of the Poisson distribution, so a χ² test on very
  long simulated runs may be slightly worse than for real data.

The source rate is constant, or decays as $R_0\,2^{-t/T_{1/2}}$ from the START of the run (the
Pa-234m button sets $T_{1/2} = 70.2$ s). In a sweep it follows a bench model: inverse square with
distance, a Gaussian angular profile and a fixed transmission per state (S1 = 1, S2 = 0.35,
S3 = 0.18, S4 = 0.08, later states 0.02). The simulator is for practising the workflow; its
numbers are not a physical model of any real setup.

---

## 10. Numerical accuracy

The core library has no dependencies and no access to the platform's `f64` functions, so `sqrt`,
`exp`, `ln`, the normal CDF $\Phi$ (through `erfc`) and the Poisson probability (through
$\ln k!$: exact factorials up to $20!$, Stirling's series above) are implemented in double-double
arithmetic and rounded once. They return the correctly rounded `f64` result, checked against
80-digit references in `crates/curie-core/tests/f64_math_tests.rs`. Rounding error is therefore
never a visible part of any number in the app; the statistical uncertainties above always dominate.
