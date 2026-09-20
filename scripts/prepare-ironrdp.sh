#!/usr/bin/env bash
# Fetch the pinned upstream dependency and apply the reviewed microphone patch.
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
parent_dir=$(dirname -- "$repo_dir")
target_dir="$parent_dir/hypr-rdp-ironrdp"
patch_file="$repo_dir/patches/ironrdp/0001-audio-input-server.patch"
base=5198cde0f73485f3fbd999ea7c49293c86b80387
patch_sha=757583b10d09a0947c9ae4da200086b9efc5363870501da6c23fa130781edb2c

# Never modify an existing checkout, including one created by an earlier run.
if [[ -e "$target_dir" || -L "$target_dir" ]]; then
    printf 'Refusing to modify existing path: %s\nUse the documented verification commands or a fresh parent directory.\n' "$target_dir" >&2
    exit 1
fi
actual_sha=$(sha256sum -- "$patch_file")
actual_sha=${actual_sha%% *}
if [[ "$actual_sha" != "$patch_sha" ]]; then
    printf 'Microphone patch checksum mismatch; refusing to continue.\n' >&2
    exit 1
fi

staging_dir=$(mktemp -d "$parent_dir/.hypr-rdp-ironrdp.XXXXXXXX")
trap 'rm -rf -- "$staging_dir"' EXIT
git -C "$staging_dir" init --quiet
git -C "$staging_dir" remote add origin https://github.com/Devolutions/IronRDP.git
git -C "$staging_dir" fetch --depth=1 origin "$base"
git -C "$staging_dir" checkout --detach FETCH_HEAD
[[ $(git -C "$staging_dir" rev-parse HEAD) == "$base" ]]
git -C "$staging_dir" apply --check "$patch_file"
git -C "$staging_dir" apply "$patch_file"
git -C "$staging_dir" add --intent-to-add .
git -C "$staging_dir" diff --check
# GNU mv -T prevents nesting; --no-clobber preserves a path created concurrently.
mv -T --no-clobber -- "$staging_dir" "$target_dir"
if [[ -d "$staging_dir" ]]; then
    printf 'Destination appeared during preparation; it was not changed.\n' >&2
    exit 1
fi
trap - EXIT
printf 'Prepared %s at upstream %s plus microphone patch.\n' "$target_dir" "$base"
