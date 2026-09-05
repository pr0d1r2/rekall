{
  # rekall's pinned toolchain. Lives at the repo ROOT, which is not
  # decoration: a flake's source root is its own directory, so a flake in a
  # subdirectory cannot see `Cargo.toml`/`src/` and can offer no package at
  # all. Root is also every Rust project's shape, so it needs no teaching.
  description = "rekall -- extract always-on agent prose into rules and skills (dev shell)";

  # hk is built by `nix-hk` and pushed to this cache. Without the substituter
  # every entry into the dev shell BUILDS hk from source, which is the cost
  # this wiring exists to avoid. Declared here rather than in each user's
  # nix.conf so the cache travels with the flake.
  nixConfig = {
    extra-substituters = [ "https://pr0d1r2.cachix.org" ];
    extra-trusted-public-keys = [
      "pr0d1r2.cachix.org-1:NfWjbhgAj41byXhCKiaE+av3Vnphm1fTezHXEGsiQIM="
    ];
  };

  # ONE nixpkgs, reached through the fleet's sole authority. This repo names
  # no revision of its own: `nixpkgs-lock` pins it (nixos-26.05, rustc 1.95)
  # and everything else FOLLOWS that lock rather than resolving a second
  # copy. A rev copied between repos is a rev that drifts the first time
  # only one of them is bumped.
  #
  # `microlith` is an input because `mth` gates this repo's SPEC.md format
  # (§C). Taking it as a flake input rather than expecting it on PATH is
  # what turns "the format check ran" from a hope into a property of the
  # shell -- the previous session deferred that check precisely because the
  # binary was missing and nothing said so (V26).
  inputs = {
    nixpkgs-lock.url = "github:pr0d1r2/nixpkgs-lock";
    nixpkgs.follows = "nixpkgs-lock/nixpkgs";
    nix-hk.url = "github:pr0d1r2/nix-hk";
    nix-hk.inputs.nixpkgs-lock.follows = "nixpkgs-lock";
    microlith.url = "github:pr0d1r2/microlith";
    microlith.inputs.nixpkgs-lock.follows = "nixpkgs-lock";
    # `itok` OWNS token accounting (V8), and `itok check` already reads a
    # `.context-limits` file and exits nonzero on a breach -- exactly the
    # runner V22 demands for that ceiling. Taking it as a GATE input rather
    # than reimplementing a counter is V8 honoured at the earliest possible
    # moment; a cargo dependency for in-crate use is a separate decision and
    # stays with T16.
    itok.url = "github:pr0d1r2/itok";
    itok.inputs.nixpkgs-lock.follows = "nixpkgs-lock";
    # `sherd` OWNS the CROSS-FILE half of the spec format. `microlith` is
    # intra-file by design -- it has no `§F` verb at all -- so a federation
    # table here would be a declaration no runner reads (V22). `sherd
    # check` walks every node, `sherd validate` proves the DAG and the
    # ceilings, and `sherd sync --check` catches a `§N` that drifted from
    # its `§F`. Taking it as a PINNED input rather than a path dependency:
    # a path dep shares a working tree, and a gate that goes green against
    # an uncommitted sibling expires silently.
    sherd.url = "github:pr0d1r2/sherd";
    sherd.inputs.nixpkgs-lock.follows = "nixpkgs-lock";
  };

  outputs =
    {
      nixpkgs,
      nix-hk,
      microlith,
      itok,
      sherd,
      ...
    }:
    let
      # `nixpkgs-lock`'s supportedSystems, all four. SUPPORT and CI COVERAGE
      # are different sets: GitHub offers no free x86_64 macOS runner, so
      # `x86_64-darwin` is supported here and unbuilt in CI, and ci.yml says
      # so rather than leaving the gap silent (SPEC.md §C, R12).
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];
      # The overlay is what makes `pkgs.hk` below mean nix-hk's hk instead of
      # nixpkgs'. Applied as an OVERLAY rather than referenced through a
      # second lookup path, so there is exactly one `pkgs` in this file.
      forAll =
        f: nixpkgs.lib.genAttrs systems (s: f (nixpkgs.legacyPackages.${s}.extend nix-hk.overlays.default));

      # rekall runs against its OWN corpus -- SPEC.md T19 makes this repo
      # consumer #0 -- so the shell must PROVIDE `rekall`, not merely the
      # toolchain to build it.
      #
      # Deliberately a SHIM rather than a package in the shell's closure: a
      # package has to BUILD the crate in order to enter the shell, so one
      # compile error locks you out of the shell you need to fix it. And a
      # built package is stale against the working tree, while `cargo run`
      # is a no-op when fresh and rebuilds only on change -- so the shim
      # always matches the source you are editing, which is the point of
      # dogfooding at all.
      rekallShim =
        pkgs:
        pkgs.writeShellScriptBin "rekall" ''
          set -eu
          manifest="''${REKALL_MANIFEST:-}"
          if [ -z "$manifest" ] || [ ! -f "$manifest" ]; then
            echo "rekall(shim): REKALL_MANIFEST unset or missing -- re-enter the dev shell from the repo root" >&2
            exit 2
          fi
          exec cargo run --quiet --manifest-path "$manifest" --bin rekall -- "$@"
        '';

      # A git hook, written BY HAND rather than by `hk install`.
      #
      # An installer writes a command that assumes its own binary is on
      # PATH. Outside this shell it is not, so an installed hook would make
      # every git command in the repo FAIL rather than merely run ungated.
      # This one tests for the runner first and degrades LOUDLY: one line to
      # stderr, exit 0.
      #
      # That is V26's split at the level of the RUNNER rather than a step: a
      # gate exists to stop bad commits, not to stop git, so the skip is
      # legal -- and the loud line is the whole safety margin, because the
      # silence is what V26 forbids.
      #
      # Materialized through the store, then COPIED into `.git/hooks` rather
      # than symlinked: a symlink into the store dangles after a GC, and a
      # dangling hook makes git error out -- the exact hard failure this
      # design exists to avoid. The shebang is `/bin/sh`, not a store path,
      # for the same reason.
      gitHook =
        pkgs: name:
        pkgs.writeText "rekall-${name}-hook" ''
          #!/bin/sh
          # Generated by rekall's dev shell (flake.nix), rewritten on every
          # entry. Edit the shellHook there, not this copy.
          command -v hk >/dev/null 2>&1 || {
            echo "hk: not on PATH -- SKIPPING the ${name} gate. Enter the dev shell (\`direnv allow\` or \`nix develop\`) to run it locally." >&2
            exit 0
          }
          exec hk run ${name} "$@"
        '';

      # The reproducible build. Possible because the flake sits at the root
      # and `Cargo.lock` is tracked -- flakes copy git-TRACKED files into the
      # store, and the build sandbox has no network, so the lock is what lets
      # nix vendor crates offline. rekall has zero dependencies today; the
      # lock still matters, because the guarantee should not depend on
      # staying dependency-free (V8 already commits to one dep later).
      rekallPkg =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "rekall";
          # Read from Cargo.toml so there is ONE version, never two that can
          # disagree.
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          # Only what the BUILD reads. With `src = ./.` every tracked file is
          # an input, so editing SPEC.md would rebuild the crate from
          # scratch -- which for a tool whose subject IS specs and prose
          # would be a rebuild on nearly every commit.
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./src
              ./Cargo.toml
              ./Cargo.lock
            ];
          };
          cargoLock.lockFile = ./Cargo.lock;
          meta = {
            description = "mine agent memory and CLAUDE.md for rules and skills, extract them, and gate that the extraction stayed honest";
            homepage = "https://github.com/pr0d1r2/rekall";
            license = pkgs.lib.licenses.mit;
            mainProgram = "rekall";
          };
        };
    in
    {
      packages = forAll (pkgs: {
        default = rekallPkg pkgs;
      });

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [
            (rekallShim pkgs)
            # `mth` from the microlith flake, not from PATH and not from a
            # cargo-run shim: this repo does not own that source, so the
            # right shape is a pinned package. `mth fmt --check` and `mth
            # check` gate SPEC.md's format (§C).
            microlith.packages.${pkgs.stdenv.hostPlatform.system}.default
            # Token counts are DELEGATED, never reimplemented here (V8).
            # `itok check` is the runner for `.context-limits` (T2, V22).
            itok.packages.${pkgs.stdenv.hostPlatform.system}.default
            # `sherd check` and `sherd validate` are the runners for the
            # federation this spec declares -- the cross-file half microlith
            # does not do.
            sherd.packages.${pkgs.stdenv.hostPlatform.system}.default
            pkgs.rustc
            pkgs.cargo
            pkgs.clippy
            pkgs.rustfmt
            pkgs.cargo-nextest
            pkgs.cargo-llvm-cov
            pkgs.llvmPackages.llvm
            pkgs.git
            # The gate runner; the ops it runs live in `hk.pkl`. From the
            # `nix-hk` overlay rather than nixpkgs: nixpkgs' hk trails the
            # releases `hk.pkl` is written against.
            pkgs.hk
            # Gate steps that need a real binary, pinned here so the dev
            # shell and CI run identical versions rather than whatever each
            # happened to install.
            pkgs.typos
            pkgs.taplo
            # `nixfmt --check` gates this very file: the flake decides what
            # every other step runs with, so drift here is drift everywhere.
            pkgs.nixfmt
            pkgs.actionlint
            # Secrets, BESIDE hk's `no-private-key` rather than instead of
            # it: that builtin does exactly what its name says and nothing
            # else, so an `AWS_SECRET_ACCESS_KEY=...` line walks past it.
            pkgs.ripsecrets
            # Relative links, in a repo that just grew a docs/ tree. Offline
            # only -- see the `links` step in hk.pkl.
            pkgs.lychee
            # The release, driven by config rather than a shell script that
            # re-implements what this already does.
            pkgs.cargo-release
            # The supply chain. This crate has FIVE direct dependencies and
            # 26 in the tree, so "there is nothing to scan" is not available
            # as an answer here the way it is in a zero-dependency sibling.
            pkgs.cargo-deny
          ];
          # Pin locale so tool output is deterministic across machines.
          LANG = "C.UTF-8";
          LLVM_COV = "${pkgs.llvmPackages.llvm}/bin/llvm-cov";
          LLVM_PROFDATA = "${pkgs.llvmPackages.llvm}/bin/llvm-profdata";
          # Resolved at ENTRY rather than baked in: `.envrc` sits beside
          # `Cargo.toml` and direnv enters with PWD set to that directory,
          # so this works wherever the repo is checked out. Derive, never
          # hardcode a path.
          #
          # Entering also installs the git hooks, unconditionally rewriting
          # them, so the hook a contributor has is always the one this file
          # describes.
          #
          # `git rev-parse --git-path hooks` rather than a literal
          # `.git/hooks`: in a worktree or a submodule `.git` is a FILE and
          # the real hooks directory is elsewhere, so the literal path
          # silently installs nothing.
          shellHook = ''
            if [ -f "$PWD/Cargo.toml" ]; then
              export REKALL_MANIFEST="$PWD/Cargo.toml"
            else
              echo "rekall(shell): no Cargo.toml in $PWD -- \`rekall\` shim disabled" >&2
            fi

            rekall_hooks="$(git rev-parse --git-path hooks 2>/dev/null || true)"
            if [ -n "$rekall_hooks" ] && [ -d "$rekall_hooks" ]; then
              install -m 755 ${gitHook pkgs "pre-commit"} "$rekall_hooks/pre-commit"
              install -m 755 ${gitHook pkgs "pre-push"} "$rekall_hooks/pre-push"
            else
              echo "rekall(shell): no git hooks directory -- hooks not installed, so nothing is gating (V22)" >&2
            fi
            unset rekall_hooks
          '';
        };
      });
    };
}
