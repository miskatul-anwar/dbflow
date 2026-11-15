#!/usr/bin/env bash
# Uploads a file to a release by its id, replacing a file of the same name.
#
#   upload-asset.sh RELEASE_ID FILE CONTENT_TYPE
#
# Releases are addressed by id, not tag: a draft release has no tag yet, and `gh release upload TAG` cannot find it.
# Needs GH_TOKEN and GITHUB_REPOSITORY.
set -euo pipefail
id="$1" file="$2" type="$3"
name="$(basename "$file")"
old="$(gh api "repos/$GITHUB_REPOSITORY/releases/$id/assets" --paginate --jq ".[] | select(.name == \"$name\") | .id")"
for asset in $old; do
  gh api -X DELETE "repos/$GITHUB_REPOSITORY/releases/assets/$asset"
done
gh api -X POST "https://uploads.github.com/repos/$GITHUB_REPOSITORY/releases/$id/assets?name=$name" \
  -H "Content-Type: $type" --input "$file" --jq '"Uploaded \(.name) (\(.size) bytes)"'
