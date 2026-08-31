# Package verification keys

`pmos@local-6a92d930.rsa.pub` is the public half of the release key used for
the current OnePlus 6T r37 APKs. `pmos@local-6a8b0868.rsa.pub` is retained for
older development/reference APKs. Publishing either public key allows package
verification; it does not reveal signing capability.

Current release-key SHA-256:
`c1f8892b9576ce1807732a985243311d272ab422fc30958a2fb78d5bfc8d36a6`

Older development-key SHA-256:
`31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6`

Never commit or copy the private `.rsa` key. The development private key is not
a suitable permanent repository root. Before a public VibeMarketOS repository
is released, rotate to a dedicated protected signing key, publish its
fingerprint through an independent channel, and document a revocation path.
