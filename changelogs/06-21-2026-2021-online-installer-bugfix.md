# Changelog

## Summary
Fixed two bugs in `online-installer.sh`: a function-name collision that caused
an infinite restart loop during installation, and missing temp subdirectory
creation that caused tar extraction to fail.

## Issues Resolved
1. **Infinite restart loop** — The bash function `install()` shadowed the
   system `/usr/bin/install` command. When the installer called `install -m 755`
   internally, bash recursively invoked the function instead of the system
   utility, resulting in an infinite loop that restarted the install sequence
   until the system ran out of resources.
2. **Tar extraction failure** — The `tar xzf ... -C "${tmpdir}/binary"` and
   `tar xzf ... -C "${tmpdir}/dot-github"` commands failed because the target
   subdirectories did not exist at extraction time. The `tar -C` flag requires
   the target directory to already exist; otherwise it exits with "Cannot open:
   No such file or directory".

## Root Causes
1. Bash function names occupy the same namespace as system commands. Defining
   `install()` overrode the system `/usr/bin/install` binary, so any call to
   `install` inside the function resolved to the function itself rather than the
   system utility, creating unbounded recursion.
2. The installer created a temp root directory via `mktemp -d` but did not
   create the nested subdirectories (`binary/`, `dot-github/`) before invoking
   `tar -C` to extract into them. `tar -C` does not create missing target
   directories; it requires them to already exist.

## Solutions
1. Renamed the function from `install()` to `do_install()`, eliminating the
   namespace collision with the system `/usr/bin/install` command.
2. Added `mkdir -p "${tmpdir}/binary"` before the binary tar extraction and
   `mkdir -p "${tmpdir}/dot-github"` before the dot-github tar extraction,
   ensuring both target directories exist before `tar -C` is invoked.

## Files Changed
- `online-installer.sh` — function renamed from `install` to `do_install`;
   added `mkdir -p` calls for `binary/` and `dot-github/` temp subdirectories.

## Status
Complete — both fixes verified present in the `online-installer-bugfix` branch.
`install()` is now `do_install()` with no residual self-recursive calls, and
both `mkdir -p` calls precede their respective `tar -C` extractions.