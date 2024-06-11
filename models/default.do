
set -eux

redo-ifchange "$2".zip
unzip >&2 "$2".zip

DIR="$(find -name 'vosk-model-small-en-us*' -type d | head -1)"
rm -rf "$2".dir
mv "$DIR" "$2".dir


echo "$2".dir >"$3"

