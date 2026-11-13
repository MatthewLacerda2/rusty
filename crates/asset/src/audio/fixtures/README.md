# Audio import fixtures (#385)

- `loop.mp3` — 0.5 s (22 050 frames) of `0.5 * sin(2π·440·t)` at 44.1 kHz, mono,
  64 kbps LAME with the LAME/Xing header that records encoder delay and padding.
  440 Hz fits exactly 220 cycles in 0.5 s, so the source loops without a seam.
  Regenerate with

      ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*440*t):s=44100:d=0.5" \
        -c:a libmp3lame -b:a 64k -fflags +bitexact -map_metadata -1 loop.mp3
