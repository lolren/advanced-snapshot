# Following GNOME Snapshot

This repository keeps two remotes:

```text
origin    https://github.com/lolren/advanced-snapshot.git
upstream  https://gitlab.gnome.org/GNOME/snapshot.git
```

The known-good base is tag `50.0`. To evaluate a newer tag without moving the
release branch:

```sh
git fetch upstream --tags
git switch -c test/upstream-TAG TAG
git cherry-pick <topical Advanced Snapshot commits in order>
```

Resolve conflicts semantically; never use a forced three-way application to
hide an API mismatch. Build and test on the host, package in a clean
postmarketOS buildroot, install into a staged generation and run all native
camera checks before updating the known-good manifest.

Generic fixes should be proposed to GNOME Snapshot or Aperture with focused
tests and without OnePlus tuning. Phone-specific defaults, the temporary
PipeWire helper and VibeMarketOS packaging stay downstream until their APIs are
accepted by the owning projects.
