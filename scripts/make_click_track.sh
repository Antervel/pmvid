#!/usr/bin/env bash
set -euo pipefail

output="${1:-click-track.mp3}"
duration="${2:-600}"

ffmpeg -y \
    -f lavfi \
    -i "aevalsrc='if(lt(mod(t\,1)\,0.05)\,0.8*sin(2*PI*1000*t)\,0)':s=44100:d=${duration}" \
    -c:a libmp3lame \
    -q:a 2 \
    "$output"

echo "wrote $output (${duration}s, one click per second)"