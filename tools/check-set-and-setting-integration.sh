#!/bin/sh
set -eu

pin=$(grep -m1 '^commit `' docs/EXAMPLE.md | sed 's/.*commit `\([^`]*\)`.*/\1/')
[ "$pin" = "9724383" ] || {
  echo 'hk: set-and-setting integration is not pinned to the measured corpus commit 9724383. Update the example and integration together.' >&2
  exit 1
}

grep -q '^      run: rekall check$' docs/set-and-setting/lefthook.yml || {
  echo 'hk: the set-and-setting lefthook fragment must run `rekall check`.' >&2
  exit 1
}
grep -q 'rekall check ||' docs/set-and-setting/hk.pkl || {
  echo 'hk: the set-and-setting hk fragment must run the pinned `rekall check`.' >&2
  exit 1
}
