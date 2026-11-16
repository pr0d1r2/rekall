#!/bin/sh
# Every file the SUITE reads through `CARGO_MANIFEST_DIR` must be inside the
# Nix package's source fileset (flake.nix, `rekallPkg`).
#
# `cargo test` runs against the working tree, where every file exists, so the
# gate goes green. `nix build .#default` runs the same suite in a sandbox that
# holds ONLY the fileset, so a test reading `.claude/commands/rekall.md` reads
# nothing and fails there -- on all four CI systems, after every local check
# passed (`src:B30`, and `.:B28` before it for `dev/`).
#
# The roots are read from flake.nix rather than restated here: a second list
# is a second rule set, and the copy is the one that rots (V23).
#
# Usage: check-package-fileset.sh [REPO_ROOT]
set -eu

root=${1:-.}
flake="$root/flake.nix"

[ -f "$flake" ] || {
  echo "hk: $flake not found, so the package fileset cannot be read." >&2
  exit 1
}

# Every `./path` between `fileset = ...unions [` and the closing `];`.
roots=$(awk '
  /fileset = pkgs\.lib\.fileset\.unions \[/ { on = 1; next }
  on && /\];/ { exit }
  on {
    line = $0
    while (match(line, /\.\/[A-Za-z0-9._\/-]+/)) {
      print substr(line, RSTART + 2, RLENGTH - 2)
      line = substr(line, RSTART + RLENGTH)
    }
  }' "$flake")

[ -n "$roots" ] || {
  echo "hk: no fileset roots found in $flake. The parse in tools/check-package-fileset.sh no longer matches \`rekallPkg\`'s src; fix the parse, do not delete the check." >&2
  exit 1
}

# Only sources INSIDE the fileset reach the sandbox; a test outside it is not
# compiled there at all, so what it reads cannot fail the package build.
files=$(for r in $roots; do
  [ -d "$root/$r" ] && find "$root/$r" -name '*.rs'
done | sort)

# For each `CARGO_MANIFEST_DIR`, the first string literal after it -- on the
# same line or, when rustfmt split the call, on the next two.
reads=$(for f in $files; do
  awk -v file="$f" '
    function first_literal(s) {
      if (match(s, /"[^"]*"/)) return substr(s, RSTART + 1, RLENGTH - 2)
      return ""
    }
    left > 0 {
      p = first_literal($0)
      if (p != "") { print file ":" at ":" p; left = 0 }
      else if (--left == 0 || $0 ~ /;[[:space:]]*$/) left = 0
    }
    index($0, "\"CARGO_MANIFEST_DIR\"") {
      rest = substr($0, index($0, "\"CARGO_MANIFEST_DIR\"") + 20)
      p = first_literal(rest)
      if (p != "") print file ":" NR ":" p
      else if (rest !~ /;[[:space:]]*$/) { left = 2; at = NR }
    }' "$f"
done)

missing=$(printf '%s\n' "$reads" | while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  path=${hit##*:}
  path=${path#/}
  covered=no
  for r in $roots; do
    case $path in
      "$r" | "$r"/*) covered=yes ;;
    esac
  done
  [ "$covered" = yes ] || echo "  $hit"
done)

[ -z "$missing" ] || {
  echo 'hk: a test reads a file the Nix package sandbox does not have. `cargo test` sees the working tree and passes; `nix build .#default` sees only the fileset in flake.nix (`rekallPkg`) and FAILS, on every CI system:' >&2
  echo "$missing" >&2
  echo 'Add the path to that fileset (`lib.fileset.maybeMissing` if it may be absent), or read the file from inside src/.' >&2
  exit 1
}
