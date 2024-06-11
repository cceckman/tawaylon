
# Hacky installer for vosk:

set -eu

redo-ifchange vosk-requirements.txt

python3 -m venv venv 1>&2
. ./venv/bin/activate
pip3 install -r vosk-requirements.txt 1>&2
cp venv/lib/*/site-packages/vosk/libvosk.so "$3"
rm -rf venv

