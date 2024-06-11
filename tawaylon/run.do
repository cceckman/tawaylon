
redo-ifchange consume.bin ../models/vosk-small

export LD_LIBRARY_PATH="$(pwd)/../venv/lib/python3.11/site-packages/vosk"
./consume.bin >&2

