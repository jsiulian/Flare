#!/bin/sh

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 <changelog file> <tag>"
  exit 1
fi

CHANGELOG_FILE=$1
CURRENT_TAG=$2
PREVIOUS_TAG=$(git describe --tags --abbrev=0 "${CURRENT_TAG}"^)

# Extract content between the two tags
awk "/^## \[${CURRENT_TAG}\] - / {flag=1; next} /^## \[${PREVIOUS_TAG}\] - / {flag=0; exit} flag" "${CHANGELOG_FILE}" |
  # Remove leading and trailing empty lines
  sed -e '1{/^$/d;}' -e '${/^$/d;}'
