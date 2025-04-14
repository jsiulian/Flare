#!/bin/sh

slug="flare"
app_id="de.schmidhuberj.Flare"
gitlab_project_id="37464215"

binary="./build/target/debug/$slug"
date=$(date +%Y-%m-%d)


if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <tag>"
  exit 1
fi



echo "> Checking git is not dirty"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "! Git is dirty. Please commit the changes before releasing."
  exit 1
fi



echo "> Updating Cargo.toml."

sed -i "s/^version = \".*\"$/version = \"$1\"/" Cargo.toml



echo "> Updating meson.build."

sed -i "s/^[[:blank:]]*version: '.*',$/  version: '$1',/" meson.build



echo "> Updating CHANGELOG.md."

# The update is not idempotent, only do it if the release does not yet exist.
if ! grep -q "## \[$1\]" CHANGELOG.md; then
  sed -i "s/## \[Unreleased\]/## \[Unreleased\]\n\n## \[$1\] - $date/" CHANGELOG.md

  sed -r -i "s|\[Unreleased\]: https://gitlab.com/schmiddi-on-mobile/$slug/-/compare/(.*)...master|\[Unreleased\]: https://gitlab.com/schmiddi-on-mobile/$slug/-/compare/$1...master\n[$1]: https://gitlab.com/schmiddi-on-mobile/$slug/-/compare/\1...$1|" CHANGELOG.md
fi



echo "> Updating metainfo."

changelog=$(bash ./build-aux/extract-changelog-for-tag.sh CHANGELOG.md $1 | md2html | sed "s/h3/p/g")
# Hack to remove newline characters; sed does not seem to like them.
changelog_newline=$(echo $changelog)

# The update is not idempotent, only do it if the release does not yet exist.
if ! grep -q "<release version=\"$1\"" "data/$app_id.metainfo.xml.in.in"; then
  # TODO: This does not format the release notes in the metainfo nicely.
  # Maybe run a formatter with the release notes?
  sed -i "s|<releases>|<releases>\n    <release version=\"$1\" date=\"$date\">\n      <description translate=\"no\">\n        $changelog_newline\n      </description>\n    </release>|" "data/$app_id.metainfo.xml.in.in"
fi



echo "> Running the application"

if command -v run 2>&1 > /dev/null; then
  run &
else
  echo "! You are not running the devshell of the flake. Please compile and start it manually (using the $binary binary)."
fi

# Wait until the application runs.
# See https://askubuntu.com/questions/1192628/how-to-create-shell-script-that-wait-till-a-program-finished-and-after-finished/1192769#1192769
# TODO: What about when testing the Flatpak?
appCount=$(ps ax | grep $binary | grep -v grep | wc -l)
while [ "$appCount" -eq "0" ]
do
  sleep 1
  appCount=$(ps ax | grep $binary | grep -v grep | wc -l)
done



echo "> The application seems to be running now. Please manually validate the release notes are correct in the about page (the displayed version will contain a git comit hash, that is expected). Press enter once confirmed."
read -n1 ans



echo "> Stop running the application now."

# Wait until the application stops.
# See https://askubuntu.com/questions/1192628/how-to-create-shell-script-that-wait-till-a-program-finished-and-after-finished/1192769#1192769
appCount=$(ps ax | grep $binary | grep -v grep | wc -l)
while [ "$appCount" -gt "0" ]
do
  sleep 1
  appCount=$(ps ax | grep $binary | grep -v grep | wc -l)
done

echo "> Application seems to be stopped now."



echo "> Validating appstream."
appstreamcli validate "build/data/$app_id.Devel.metainfo.xml"

if [ $? -ne 0 ]; then
  echo "! Appstream validation failed. Please fix it and redo the release."
  exit 1
fi



echo "> Doing the commit."
git add .
git commit



echo "> Tagging the latest commit."
git tag $1



echo "> Displaying what was changed."
git show HEAD



echo "> Press 'Enter' when you are ready to push the repository. This can not be undone!"
read -n1 ans
git push && git push --tags



echo "> Waiting for CI to finish. This will take approximately 30 minutes. You will be notified once CI is finished."

until curl -I --fail "https://gitlab.com/api/v4/projects/${gitlab_project_id}/packages/generic/$slug/$1/$slug-$1.tar.xz" &> /dev/null; do
  # Try again in 2 minutes
  sleep 120
done



notify-send "CI is finished" "Please continue with the instructions provided for releasing the new version."
echo "> Please create the PR to update the Flatpak."

# TODO: This is wrong if there is already a PR open.
pr_number=$(curl "https://api.github.com/repos/flathub/$app_id/pulls" 2> /dev/null | jq -r '.[0].number')
while [ "$pr_number" = "null" ]; do
  # Try again in 2 minutes
  sleep 120
  pr_number=$(curl "https://api.github.com/repos/flathub/$app_id/pulls" 2> /dev/null | jq -r '.[0].number')
done



echo "> Detected open Flatpak PR to monitor: $pr_number. Depending on the current Flathub buildbot usage, the build can take a long time to complete. You will be notified once the build succeeds."

comments=$(curl "https://api.github.com/repos/flathub/$app_id/issues/$pr_number/comments" 2> /dev/null)
while ! echo "$comments" | grep "Build.*successful" &> /dev/null; do
  if echo "$comments" | grep "Build.*failed" &> /dev/null; then
    notify-send "Flathub build failed" "Please resolve manually."
    echo "! Flathub build failed. Please resolve manually."
    exit 1
  fi
  # Try again in 2 minutes
  sleep 120
  comments=$(curl "https://api.github.com/repos/flathub/$app_id/issues/$pr_number/comments" 2> /dev/null)
done



notify-send "Flathub build successful" "Please merge the PR."
echo "> Flathub build successful. Please merge the PR."

status=$(curl "https://api.github.com/repos/flathub/$app_id/pulls/$pr_number" 2> /dev/null | jq -r '.state')
while [ "$status" = "open" ]; do
  # Try again in 2 minutes
  sleep 120
  status=$(curl "https://api.github.com/repos/flathub/$app_id/pulls/$pr_number" 2> /dev/null | jq -r '.state')
done



notify-send "Flathub PR merged" "Please continue with the instructions provided for releasing the new version."

echo "> You are all set now. Please still do the following things afterwards:"
echo ">> Write a release message to the Matrix channel."
echo ">> If it was a bigger repease, consider doing a Mastodon announcement."
