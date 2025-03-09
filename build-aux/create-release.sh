#!/bin/sh

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <tag>"
  exit 1
fi

echo "> Checking git is not dirty"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "! Git is dirty. Please commit the changes before releasing."
  exit 1
fi

date=$(date +%Y-%m-%d)

echo "> Updating Cargo.toml"

sed -i "s/^version = \".*\"$/version = \"$1\"/" Cargo.toml



echo "> Updating meson.build"

sed -i "s/^[[:blank:]]*version: '.*',$/  version: '$1',/" meson.build



echo "> Updating CHANGELOG.md"

# The update is not idempotent, only do it if the release does not yet exist.
if ! grep -q "## \[$1\]" CHANGELOG.md; then
  sed -i "s/## \[Unreleased\]/## \[Unreleased\]\n\n## \[$1\] - $date/" CHANGELOG.md

  sed -r -i "s|\[Unreleased\]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/(.*)...master|\[Unreleased\]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/$1...master\n[$1]: https://gitlab.com/schmiddi-on-mobile/flare/-/compare/\1...$1|" CHANGELOG.md
fi



echo "> Updating data/de.schmidhuberj.Flare.metainfo.xml.in.in"

changelog=$(bash ./build-aux/extract-changelog-for-tag.sh CHANGELOG.md $1 | md2html | sed "s/h3/p/g")
# Hack to remove newline characters; sed does not seem to like them.
changelog_newline=$(echo $changelog)

# The update is not idempotent, only do it if the release does not yet exist.
if ! grep -q "<release version=\"$1\"" data/de.schmidhuberj.Flare.metainfo.xml.in.in; then
  # TODO: This does not format the release notes in the metainfo nicely.
  # Maybe run a formatter with the release notes?
  sed -i "s|<releases>|<releases>\n    <release version=\"v$1\" date=\"$date\">\n      <description translate=\"no\">\n        $changelog_newline\n      </description>\n    </release>|" data/de.schmidhuberj.Flare.metainfo.xml.in.in
fi



echo "> Updating data/resources/ui/about.blp.in"

sed -i "s|release-notes: .*|release-notes: \"$changelog_newline\";|" data/resources/ui/about.blp.in



echo "> Running Flare"

if command -v run 2>&1 > /dev/null; then
  run &
else
  echo "! You are not running the devshell of the flake. Please compile and start it manually. (using the ./build/target/debug/flare binary)"
fi

# Wait until Flare runs.
# See https://askubuntu.com/questions/1192628/how-to-create-shell-script-that-wait-till-a-program-finished-and-after-finished/1192769#1192769
# TODO: What about when testing the Flatpak?
appName="./build/target/debug/flare"
appCount=$(ps ax | grep $appName | grep -v grep | wc -l)
while [ "$appCount" -eq "0" ]
do
  sleep 1
  appCount=$(ps ax | grep $appName | grep -v grep | wc -l)
done

echo "> Flare seems to be running now."

echo "> Please manually validate the release notes are correct in the about page (the release version will display a git comit hash, that is expected). Press enter once confirmed."
read -n1 ans

echo "> Stop running Flare now."

# Wait until Flare stops .
# See https://askubuntu.com/questions/1192628/how-to-create-shell-script-that-wait-till-a-program-finished-and-after-finished/1192769#1192769
appCount=$(ps ax | grep $appName | grep -v grep | wc -l)
while [ "$appCount" -gt "0" ]
do
  sleep 1
  appCount=$(ps ax | grep $appName | grep -v grep | wc -l)
done

echo "> Flare seems to be stopped now."



echo "> Validating appstream"
appstreamcli validate "build/data/de.schmidhuberj.Flare.Devel.metainfo.xml"

if [ $? -ne 0 ]; then
  echo "! Appstream validation failed. Please fix it and redo the release."
  exit 1
fi



echo "> Doing the commit"
git add .
git commit



echo "> Tagging the latest commit"
git tag $1



echo "> You are all set now. Please still do the following things afterwards:"
echo ">> Push the release using 'git push --follow-tags'."
echo ">> Wait for CI to finish (will take approximately 30 minutes)."
echo ">> Release the new version on Flathub."
echo ">> Write a release message to the Matrix channel."
