#!/usr/bin/env bash

echo "Expected"
cat ../correct

echo
file=$(../target/debug/codecrafters-git write-tree)
echo "Actual ($file)"
parent="${file:0:2}"
obj="${file:2}"
python3 -c "import zlib, sys; sys.stdout.buffer.write(zlib.decompress(open(sys.argv[1], 'rb').read()))" ".git/objects/${parent}/${obj}" | hexdump -c

