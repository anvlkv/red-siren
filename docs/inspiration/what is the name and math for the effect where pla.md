<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# what is the name and math for the effect where playing keys on a piano also triggers some of the adjacent keys.

The effect is usually called **sympathetic resonance** or **string resonance** in a piano. It happens when a played note excites other strings whose natural frequencies line up with harmonics of the note being played, especially when their dampers are lifted.[^1_1][^1_4]

The basic math is resonance between oscillators. A string’s resonant frequencies are approximately

$$
f_n = n \cdot \frac{1}{2L}\sqrt{\frac{T}{\mu}}
$$

where $L$ is string length, $T$ is tension, $\mu$ is linear mass density, and $n=1,2,3,\dots$. If another string has a mode near one of those frequencies, energy transfers efficiently and it starts vibrating too.[^1_2][^1_1]

A useful way to think about the coupling is with forced oscillation:

$$
m\ddot{x} + c\dot{x} + kx = F_0 \cos(\omega t)
$$

The response becomes large when the driving frequency $\omega$ is near the system’s natural frequency $\omega_0 = \sqrt{k/m}$, which is the resonance condition.[^1_9][^1_1]
<span style="display:none">[^1_10][^1_3][^1_5][^1_6][^1_7][^1_8]</span>

<div align="center">⁂</div>

[^1_1]: https://pub.dega-akustik.de/ICA2019/data/articles/001474.pdf

[^1_2]: https://www.kvraudio.com/forum/viewtopic.php?t=571562

[^1_3]: https://www.reddit.com/r/classicalmusic/comments/twdcx0/piece_that_make_use_of_silently_pressed_keys/

[^1_4]: https://www.youtube.com/watch?v=c2VHVjHxvq0

[^1_5]: https://www.youtube.com/shorts/lWAXOR1lCiQ

[^1_6]: https://www.pianonoise.com/Article.overtone-series.htm

[^1_7]: https://proaudiogarden.com/sympathetic-resonance/

[^1_8]: https://www.reddit.com/r/piano/comments/18eolbp/note_resonates_when_the_same_pitch_in_a_different/

[^1_9]: https://en.wikipedia.org/wiki/Overtone

[^1_10]: https://www.key-notes.com/blog/what-is-sympathetic-resonance


---

# say I can hit a key with more or less strength. how to compute the strength of the effect too?

Yes — the **strength** is usually modeled as a resonance-coupling problem, where the effect gets stronger the closer the played note is to the other string’s natural frequencies and the more energy is injected into the driven string.[^2_1][^2_2]

## A practical model

A common first approximation is a forced, damped oscillator:

$$
m\ddot x + c\dot x + kx = F(t)
$$

Here $F(t)$ is the hammer strike on the played string, and the adjacent string responds through coupling or sound-field excitation. The response amplitude near a resonant frequency $\omega_0$ is often written as[^2_3][^2_4]

$$
A(\omega) \propto \frac{F_0}{\sqrt{(\omega_0^2-\omega^2)^2 + (2\zeta\omega_0\omega)^2}}
$$

so the response grows when the drive frequency $\omega$ is close to $\omega_0$, and it gets larger when damping $\zeta$ is smaller.[^2_2][^2_1]

## Mapping key strength to effect strength

If by “more or less strength” you mean how hard you hit the key, then the hammer’s strike velocity is usually the key variable, not just finger force. In piano acoustics, louder playing comes from a faster hammer strike, which changes the force waveform and injects more energy into the string. A simple scaling is:[^2_3]

$$
E_{\text{input}} \approx \frac{1}{2}m_{\text{hammer}}v^2
$$

so if hammer speed $v$ doubles, input energy goes up by about $4\times$. A useful proxy for sympathetic strength is then

$$
S \propto E_{\text{input}} \cdot C(\Delta f)
$$

where $C(\Delta f)$ is a coupling term that falls off as the frequency mismatch $\Delta f$ between the struck note and the adjacent string increases.[^2_5][^2_1]

## Simple tuning-sensitive formula

For two nearby strings, a compact “effect strength” model is:

$$
S = K \cdot v^2 \cdot \frac{1}{1 + \left(\frac{\Delta f}{\gamma}\right)^2}
$$

- $v$ = hammer strike speed.
- $\Delta f$ = difference between the note’s harmonic and the neighboring string’s resonant frequency.
- $\gamma$ = resonance bandwidth.
- $K$ = constant capturing coupling, soundboard effects, and damper state.

This says the effect gets stronger with harder strikes and gets much weaker as the neighboring string moves away from exact harmonic alignment.[^2_4][^2_1]

## For a more physical simulation

If you want to compute it accurately, model the piano as coupled damped oscillators:

$$
\mathbf{M}\ddot{\mathbf{x}} + \mathbf{C}\dot{\mathbf{x}} + \mathbf{K}\mathbf{x} = \mathbf{f}(t)
$$

Then define the “effect strength” as something like

$$
S = \int_0^T x_{\text{adj}}^2(t)\,dt
$$

or peak amplitude of the adjacent string displacement $x_{\text{adj}}(t)$. That gives you a measurable output you can compare across strike strengths and notes.[^2_2][^2_3]

If you want, I can turn this into a small simulation formula or Rust/Python code for a piano resonance toy model.
<span style="display:none">[^2_10][^2_6][^2_7][^2_8][^2_9]</span>

<div align="center">⁂</div>

[^2_1]: https://en.wikipedia.org/wiki/Sympathetic_resonance

[^2_2]: https://pub.dega-akustik.de/ICA2019/data/articles/001474.pdf

[^2_3]: https://euphonics.org/11-2-hitting-strings-the-piano-and-its-relatives/

[^2_4]: https://www.sciencelearn.org.nz/resources/2815-sound-resonance

[^2_5]: https://www.kvraudio.com/forum/viewtopic.php?t=571562

[^2_6]: https://proaudiogarden.com/sympathetic-resonance/

[^2_7]: https://www.youtube.com/shorts/lWAXOR1lCiQ

[^2_8]: https://www.facebook.com/groups/876194196241093/posts/1265851313942044/

[^2_9]: https://www.youtube.com/watch?v=c2VHVjHxvq0

[^2_10]: https://www.facebook.com/groups/physicsisfun109/posts/799203466091833/


---

```
# <q>A useful way to think about the coupling is with forced oscillation:mx¨+cx˙+kx=F0cos⁡(ωt)m\ddot{x} + c\dot{x} + kx = F_0 \cos(\omega t)mx¨+cx˙+kx=F0​cos(ωt)The response becomes large when the driving frequency ω\omegaω is near the system’s natural frequency ω0=k/m\omega_0 = \sqrt{k/m}ω0​=k/m​,</q> what are k, x, m in these formulas
```

In that equation:

- $x$ is the **displacement** from equilibrium, meaning how far the oscillator has moved from its resting position.
- $m$ is the **mass** of the object that is moving.
- $k$ is the **spring constant** or **stiffness**, which tells you how strongly the system resists being displaced.

So in the piano analogy, $x$ would be how much a string moves, $m$ is the effective vibrating mass of the string, and $k$ is its stiffness/tension-related restoring strength.[^3_1][^3_5][^3_6]

In the formula $m\ddot{x} + c\dot{x} + kx = F_0\cos(\omega t)$, the terms mean:

- $m\ddot{x}$: inertia, the resistance to acceleration.
- $c\dot{x}$: damping, energy lost to friction and air.
- $kx$: restoring force, proportional to displacement.[^3_5][^3_1]

A compact way to remember it is: **mass sets how hard it is to speed up, stiffness sets how hard it is to pull away, and displacement is the current offset**.[^3_6][^3_9]
<span style="display:none">[^3_2][^3_3][^3_4][^3_7][^3_8]</span>

<div align="center">⁂</div>

[^3_1]: https://courses.lumenlearning.com/suny-osuniversityphysics/chapter/15-6-forced-oscillations/

[^3_2]: https://www.math.utah.edu/~gustafso/f2010/forcedVibrationsUndamped2008.pdf

[^3_3]: https://web.uvic.ca/~tbazett/diffyqs/forcedo_section.html

[^3_4]: https://ocw.mit.edu/courses/8-03sc-physics-iii-vibrations-and-waves-fall-2016/782069da3820fc514c10c26ae0c15b01_MIT8_03SCF16_Text_Ch2.pdf

[^3_5]: https://phys.libretexts.org/Bookshelves/University_Physics/University_Physics_(OpenStax)/Book:_University_Physics_I_-_Mechanics_Sound_Oscillations_and_Waves_(OpenStax)/15:_Oscillations/15.07:_Forced_Oscillations

[^3_6]: https://en.wikipedia.org/wiki/Harmonic_oscillator

[^3_7]: https://lehman.edu/faculty/anchordoqui/chapter23.pdf

[^3_8]: https://www.dbscience.org/wp-content/uploads/2020/03/BSC-SEM-II-PAPER-I-UNIT-II-FORCED-OSCILLATOR.pdf

[^3_9]: http://hyperphysics.phy-astr.gsu.edu/hbase/shm2.html

