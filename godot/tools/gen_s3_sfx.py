#!/usr/bin/env python3
"""Synthesises the four S3 placeholder sounds (docs/slices/S3-armour.md
"Presenter", audio) with the Python standard library and encodes them to
Ogg Vorbis with ffmpeg:

    cannon.ogg         tank main gun: crack, low boom, rumbling tail
    engine_loop.ogg    tracked-vehicle engine, a seamless 2 s loop
    ap_impact.ogg      AP round striking steel: a clang of inharmonic partials
    rocket_launch.ogg  AT rocket: pop and a falling whoosh

They are our own work (no samples, fixed seeds), released as CC0-1.0, and
each gets a `<file>.license.toml` like the downloaded S1 sounds
(SYSTEM_DESIGN.md §12.1). Re-running reproduces the WAV data exactly; the
Vorbis encoder may differ between ffmpeg builds, so the manifests record
both hashes.

    python3 -I godot/tools/gen_s3_sfx.py [out_dir]   # default godot/audio/sfx
"""

import hashlib
import math
import os
import random
import struct
import subprocess
import sys
import tempfile
import wave

RATE = 22050


def lowpass(xs, cutoff):
    """One-pole low-pass."""
    a = 1.0 - math.exp(-2.0 * math.pi * cutoff / RATE)
    y = 0.0
    out = []
    for x in xs:
        y += a * (x - y)
        out.append(y)
    return out


def highpass(xs, cutoff):
    lp = lowpass(xs, cutoff)
    return [x - l for x, l in zip(xs, lp)]


def noise(n, seed):
    r = random.Random(seed)
    return [r.uniform(-1.0, 1.0) for _ in range(n)]


def normalise(xs, peak=0.89):
    m = max(abs(x) for x in xs) or 1.0
    return [x * peak / m for x in xs]


def fade(xs, fade_in, fade_out):
    n = len(xs)
    fi, fo = int(fade_in * RATE), int(fade_out * RATE)
    out = list(xs)
    for i in range(fi):
        out[i] *= i / fi
    for i in range(fo):
        out[n - 1 - i] *= i / fo
    return out


def cannon():
    n = int(1.9 * RATE)
    crack = highpass(noise(n, 11), 900.0)
    rumble = lowpass(lowpass(noise(n, 12), 160.0), 160.0)
    out = []
    phase = 0.0
    for i in range(n):
        t = i / RATE
        f = 38.0 + 52.0 * math.exp(-t / 0.12)
        phase += 2.0 * math.pi * f / RATE
        boom = math.sin(phase) * math.exp(-t / 0.35)
        out.append(
            1.6 * crack[i] * math.exp(-t / 0.018)
            + 1.0 * boom
            + 5.0 * rumble[i] * math.exp(-t / 0.6) * min(t / 0.03, 1.0)
        )
    return fade(normalise(out), 0.0, 0.3)


def engine_loop():
    # 2.0 s; every partial completes whole cycles (multiples of 0.5 Hz)
    # and the noise is filtered circularly, so the loop has no seam.
    seconds = 2.0
    n = int(seconds * RATE)
    base = 31.0  # firing frequency at idle, Hz
    parts = [(1, 1.0), (2, 0.55), (3, 0.35), (4, 0.2), (6, 0.12), (8, 0.07)]
    out = [0.0] * n
    for k, amp in parts:
        f = base * k
        for i in range(n):
            out[i] += amp * math.sin(2.0 * math.pi * f * i / RATE + k * 0.7)
    # Cylinder-firing roughness: amplitude modulation at 15.5 Hz.
    for i in range(n):
        out[i] *= 0.75 + 0.25 * math.sin(2.0 * math.pi * 15.5 * i / RATE)
    nz = noise(n, 21)
    # Circular low-pass: run twice, keep the second pass.
    lp = lowpass(nz + nz, 420.0)[n:]
    clank = highpass(noise(n, 22), 2500.0)
    for i in range(n):
        tick = 0.5 + 0.5 * math.sin(2.0 * math.pi * 7.5 * i / RATE)
        out[i] += 0.9 * lp[i] + 0.05 * clank[i] * tick**8
    return normalise(out, 0.7)


def ap_impact():
    n = int(1.2 * RATE)
    click = highpass(noise(n, 31), 1500.0)
    partials = [(523.0, 1.0, 0.35), (1341.0, 0.7, 0.22), (2209.0, 0.5, 0.15),
                (3467.0, 0.35, 0.09), (4920.0, 0.2, 0.05)]
    thud = lowpass(noise(n, 32), 220.0)
    out = []
    for i in range(n):
        t = i / RATE
        s = 1.4 * click[i] * math.exp(-t / 0.006) + 6.0 * thud[i] * math.exp(-t / 0.08)
        for f, a, d in partials:
            s += a * math.sin(2.0 * math.pi * f * t) * math.exp(-t / d)
        out.append(s)
    return fade(normalise(out), 0.0, 0.1)


def rocket_launch():
    n = int(1.5 * RATE)
    nz = noise(n, 41)
    pop = lowpass(noise(n, 42), 300.0)
    out = []
    lo = hi = 0.0
    for i in range(n):
        t = i / RATE
        # A band of noise sliding down from about 3 kHz to 500 Hz.
        fc = 500.0 + 2500.0 * math.exp(-t / 0.5)
        a_hi = 1.0 - math.exp(-2.0 * math.pi * fc * 1.6 / RATE)
        a_lo = 1.0 - math.exp(-2.0 * math.pi * fc * 0.6 / RATE)
        hi += a_hi * (nz[i] - hi)
        lo += a_lo * (nz[i] - lo)
        band = hi - lo
        env = min(t / 0.04, 1.0) * math.exp(-t / 0.55)
        out.append(2.5 * band * env + 4.0 * pop[i] * math.exp(-t / 0.05))
    return fade(normalise(out), 0.0, 0.25)


def write_wav(path, xs):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1.0, min(1.0, x)) * 32767)) for x in xs))


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


MANIFEST = """# Licence manifest for {name} (SYSTEM_DESIGN.md §12.1 step 3).
source_url = "generated: godot/tools/gen_s3_sfx.py ({fn}())"
file_url = ""
platform_id = "generated"
uploader = "Frontline RTS (S3 presenter build)"
author = "Frontline RTS project (procedural synthesis, no samples)"
spdx = "CC0-1.0"
downloaded = 2026-11-01
sha256 = "{ogg}"
sha256_original = "{wav}"
sha256_derived = ["{ogg}"]
parents = []
derivation = "{what}; Python stdlib synthesis at 22.05 kHz mono (fixed seeds), ffmpeg native Vorbis q4"
noai = false
mod_redistributable = "yes"
reviewer = ""
s86a_check = "n/a (audio)"
period_check = "n/a"
"""

SOUNDS = [
    ("cannon.ogg", cannon, "tank gun: noise crack, 90->38 Hz boom, low rumble tail, 1.9 s"),
    ("engine_loop.ogg", engine_loop, "engine: 31 Hz firing harmonics with roughness and filtered noise, seamless 2 s loop"),
    ("ap_impact.ogg", ap_impact, "AP on steel: click, thud and five inharmonic decaying partials, 1.2 s"),
    ("rocket_launch.ogg", rocket_launch, "AT rocket: low pop and a falling band of noise, 1.5 s"),
]


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "audio", "sfx")
    with tempfile.TemporaryDirectory() as tmp:
        for name, fn, what in SOUNDS:
            wav = os.path.join(tmp, name.replace(".ogg", ".wav"))
            write_wav(wav, fn())
            ogg = os.path.join(out_dir, name)
            subprocess.run(
                ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", wav,
                 "-c:a", "vorbis", "-strict", "-2", "-q:a", "4", "-ac", "2", ogg],
                check=True,
            )
            with open(ogg + ".license.toml", "w", encoding="utf-8") as f:
                f.write(MANIFEST.format(name=name, fn=fn.__name__, ogg=sha(ogg), wav=sha(wav), what=what))
            print("%s: %d bytes" % (name, os.path.getsize(ogg)))


if __name__ == "__main__":
    main()
