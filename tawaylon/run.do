
redo-ifchange vosk-example ../models/vosk-small

export LD_LIBRARY_PATH="$(pwd)/../venv/lib/python3.11/site-packages/vosk"
./vosk-example >&2

