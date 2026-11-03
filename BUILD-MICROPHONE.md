# Build the microphone development version

Clone this community fork (the upstream package does not include these changes):

```bash
git clone https://github.com/ustaaa/hypr-rdp-plus.git hypr-rdp-microphone
cd hypr-rdp-microphone
```

Keep this repository and its patched IronRDP dependency next to each other:

```text
work/
  hypr-rdp-microphone/
  hypr-rdp-ironrdp/
```

Install the normal Hypr-RDP build prerequisites described in `README.md`, plus
Git, Bash and GNU coreutils. Use a Rust toolchain meeting the dependency's
minimum supported Rust version. From this repository's root, run:

```bash
./scripts/prepare-ironrdp.sh
cargo build --release --locked
```

The script downloads the exact upstream IronRDP base commit, checks the bundled
patch checksum, and applies the patch in a temporary sibling directory before
placing it at `../hypr-rdp-ironrdp`. Existing directories or symlinks are refused;
the script never resets, deletes or modifies an existing dependency checkout.
Use a fresh parent directory if you need an independent reproducibility check.
The resulting uncommitted dependency changes are intentional.

The pinned base is `5198cde0f73485f3fbd999ea7c49293c86b80387`. Verify a checkout
created by this script without altering it:

```bash
git -C ../hypr-rdp-ironrdp rev-parse HEAD
git -C ../hypr-rdp-ironrdp diff --check
git -C ../hypr-rdp-ironrdp apply --reverse --check \
  "$PWD/patches/ironrdp/0001-audio-input-server.patch"
```

The first command must print the pinned base. The other two must succeed.
The reverse-apply check confirms the patch is present, but does not prove the
absence of unrelated changes; inspect `git diff` and `git status` if provenance
is uncertain. Local development checkouts may instead contain the patch as
commits; those have a different HEAD and should be reviewed separately.

See `patches/ironrdp/README.md` for upstream authorship, PRs and patch provenance.
See `MICROPHONE.md` for runtime configuration and testing. Building does not
install the binary, restart services, or change the active desktop session.
