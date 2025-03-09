#!/bin/sh

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 <changelog file> <tag>"
  exit 1
fi

CHANGELOG_FILE=$1
CURRENT_TAG=$2

# Extract content between the tag and the next occurance of "## [".
awk "/^## \[${CURRENT_TAG}\] - / {flag=1; next} /^## \[/ && (flag==1) {flag=0; exit} flag" "${CHANGELOG_FILE}" |
  # Remove leading and trailing empty lines
  sed -e '1{/^$/d;}' -e '${/^$/d;}'
