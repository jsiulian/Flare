#!/bin/sh

missing=0

lines=$(fd -t f -e blp . data/)
lines+=$'\n'
lines+=$(fd -t f -e rs . src/)

while read -r line; do
  grep -q "$line" po/POTFILES
  if [ $? -ne 0 ]; then
      echo "Missing: $line"
      missing=1
  fi
done <<< "$lines"

if [ $missing -ne 0 ]; then
  exit 1
fi
