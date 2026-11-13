"""Regenerate tone.mp3, the MP3 decode fixture (#545).

    pip install lameenc==1.8.4 && python3 src/audio/device/fixtures/make_tone_mp3.py

250 ms of a 440 Hz sine at half scale, mono, 44.1 kHz in, 32 kbps CBR (LAME
resamples it to 22.05 kHz). The PCM is a
pure function of the constants below, so a rerun with the same lameenc is
byte-identical.
"""
import math
import pathlib
import struct

import lameenc

RATE, SECONDS, FREQ = 44_100, 0.25, 440.0
pcm = b"".join(
    struct.pack("<h", int(16_383 * math.sin(2 * math.pi * FREQ * i / RATE)))
    for i in range(int(RATE * SECONDS))
)
enc = lameenc.Encoder()
enc.set_bit_rate(32)
enc.set_in_sample_rate(RATE)
enc.set_channels(1)
enc.set_quality(2)
mp3 = enc.encode(pcm) + enc.flush()
pathlib.Path(__file__).with_name("tone.mp3").write_bytes(mp3)
