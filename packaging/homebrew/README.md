# Homebrew channel

Status: the canonical `wavect/tap` channel serves v0.9.0 on Apple Silicon and
Intel macOS. The generated recipe is retained under
[`../releases/v0.9.0/homebrew`](../releases/v0.9.0/homebrew).
[`../channels.json`](../channels.json) records the last publicly verified version.

The package is a binary formula, `semaprax`, in the tap `wavect/tap`
(repository `wavect/homebrew-tap`, formula `Formula/semaprax.rb`). It installs the
same published archive as the manual route: `semaprax` (the full build) and
`semapraxd`, plus `LICENSE` and the per-archive `release-manifest.json`. It never
compiles the workspace and never runs `install.sh`.

## Generate a formula for a release

Only from a release that is already published, with its own `SHA256SUMS` and
aggregate `release-manifest.json`:

```sh
gh release download v0.9.0 -R wavect/semaprax -p SHA256SUMS -p release-manifest.json
python3 scripts/channel-manifests.py homebrew --tag v0.9.0 \
  --sums SHA256SUMS --manifest release-manifest.json --out out
# out/homebrew/semaprax.rb
```

The generator fails closed (writes nothing) if the manifest tag/version, the
archive name or either digest disagree, or if no macOS archive exists. Output is
deterministic, so rerunning for the same release is a no-op diff. The formula
covers macOS only by default (Apple Silicon for v0.8.0, plus Intel macOS once a
release publishes that archive). `--include-linux` adds `on_linux` blocks, but
Homebrew on Linux links the host glibc, so use it only after a `brew test` on a
host at the archive's glibc baseline.

## Update process after each release

1. Wait for the release to be published and its hosted evidence recorded.
2. Generate the formula as above and diff it against the tap's current
   `Formula/semaprax.rb`.
3. Validate locally in a throwaway tap, then open a pull request on
   `wavect/homebrew-tap`:

   ```sh
   brew tap-new wavect/semaprax-localtest --no-git
   cp out/homebrew/semaprax.rb "$(brew --repository wavect/semaprax-localtest)/Formula/"
   brew install --formula wavect/semaprax-localtest/semaprax
   brew test wavect/semaprax-localtest/semaprax
   brew audit --strict wavect/semaprax-localtest/semaprax
   brew uninstall --force semaprax && brew untap wavect/semaprax-localtest
   ```

   `brew audit` needs `homebrew/core` available locally; untap it again if you did
   not have it before.
4. Merge the tap change, then verify the public command in a fresh shell:
   `brew install wavect/tap/semaprax`, `brew test semaprax`.
5. Only then set `homebrew.status` to `published` (with `version`, the release
   tag, and `verified_at`) in `packaging/channels.json` and add the user-facing
   install, upgrade and uninstall commands to the install guide. A delayed or
   rejected tap update leaves the previous formula installable and the docs
   unchanged.

The public v0.9.0 formula passed native installation, `brew test` (starter
printing `42`, both executables present), `brew audit --strict` and removal on
macOS 15 arm64 and x86-64 in [run 38156173827](https://github.com/wavect/semaprax/actions/runs/38156173827),
after the tap update at `360054d96abbf684d9b31f0e657a88f6b1eaf4da`.
The preceding [run 38155570863](https://github.com/wavect/semaprax/actions/runs/38155570863)
also upgraded the real Apple Silicon v0.8.0 formula to the v0.9.0 proposal.
The Package channels workflow retains deterministic proposals from published,
attested bytes; publishing the reviewed tap update is a separate operation.
