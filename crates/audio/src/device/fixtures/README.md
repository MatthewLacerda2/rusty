# Audio decode fixtures

- `tone.mp3` — 250 ms, 440 Hz, mono, half scale. Regenerate with `make_tone_mp3.py` (#545).
- `tone.ogg` — 250 ms, 440 Hz, stereo Vorbis at 22.05 kHz: left at half scale, right
  at quarter scale, so a test can tell the channels apart (#465). Regenerate with

      ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*440*t)|0.25*sin(2*PI*440*t):s=22050:d=0.25" \
        -c:a libvorbis -q:a 0 -fflags +bitexact -map_metadata -1 tone.ogg
