Ukulele
-------

Version 2026-08-11

Recorded by Mateusz Dąbrowski from a Flight Fireball ukulele (tenor). Offered to FreePats on August 2026.

Published under the terms of Creative Commons CC0 public domain dedication:
https://creativecommons.org/publicdomain/zero/1.0/

Includes 13 pitched samples, one per position, plus two percussive articulations that are part of how a ukulele is actually played:

 - body-tap.wav: a knock on the instrument body (0.31 s)
 - chuck.wav: a strum into damped strings - the percussive "chk" of a muted stroke (0.16 s)

Source: RØDE Podcaster mic, Elgato Wave XLR, captured at 96 kHz / 24-bit mono, one room, one session, fixed mic position and gain throughout.

Processing applied:

 - Resampled 96 kHz -> 48 kHz (afconvert, highest quality setting). 24-bit depth carried through from the original capture; nothing has been through a 16-bit stage.

 - 6th-order Butterworth high-pass at 190 Hz on the pitched samples. A refrigerator compressor in an adjacent room put tonal noise at 105-140 Hz, close enough to need a steep filter rather than a gentle one. 190 Hz still sits below the lowest fundamental here (C4, 261.6 Hz), where the filter costs 0.09 dB. Residual energy in the 105-140 Hz band now sits 23-36 dB below each sample's own peak, and the noise floor measures -68.9 dBFS.

 - body-tap.wav is from the same session and is filtered the same way. chuck.wav is NOT filtered: it comes from a separate session recorded with the fridge switched off, so there was nothing to remove - and a meaningful share of a chuck's energy sits low, which is part of the sound rather than noise.

 - Trimmed to the full natural decay, per note rather than a fixed length: the scan runs until a 50 ms RMS window reaches twice the measured noise floor. That gives 3.25 s at the bottom of the range down to 1.10 s at the top, which is the instrument - a soprano's short scale gives the upper positions genuinely less sustain. These are longer than the versions in the app, which truncates for size.

 - 150 ms fade at each tail so the sample end cannot click.

 - Levels matched across notes on a 300 ms window from the attack, so the set plays evenly across the range without per-note adjustment in the SFZ, then one global gain (+5.2 dB) so the loudest peak lands at -3 dBFS. Peaks run -3.0 to -8.4 dBFS; the spread is attack transient, not loudness.

 - The two percussive samples are each scaled to -3 dBFS individually, since they are not part of that pitched balance.
