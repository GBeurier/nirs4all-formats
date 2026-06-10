---
orphan: true
---

# Encrypted local-only sample archive

`samples_local/` holds the non-redistributable vendor corpora (license-restricted,
login-gated, or customer data) that drive the local-only reader tests. The raw tree is
gitignored and never committed. To keep a recoverable, versioned copy *next to the code*
without publishing the files, the whole tree is packed into a single AES-256 archive that
**is** committed:

| Path | Committed? | Role |
|---|---|---|
| `samples_local/` | no (gitignored) | the working tree of raw fixtures |
| `samples_local.tar.gz.enc` | **yes** | `gzip`-ed tarball, AES-256-CBC + PBKDF2 (200k iters, salted) |
| `samples_local.key` | no (gitignored) | the base64 passphrase — the only thing that stays out of git |
| `scripts/samples_local_crypt.sh` | yes | encrypt / decrypt / verify helper |

## Restore the corpus on a fresh checkout

You need `samples_local.key` (shared out-of-band between maintainers, kept locally in the
repo root). Then:

```bash
./scripts/samples_local_crypt.sh decrypt   # samples_local.tar.gz.enc -> samples_local/
```

## Re-encrypt after changing fixtures

```bash
./scripts/samples_local_crypt.sh encrypt   # rewrites samples_local.tar.gz.enc from samples_local/
./scripts/samples_local_crypt.sh verify    # decrypts to a temp dir and diffs against samples_local/
git add samples_local.tar.gz.enc           # commit the refreshed blob
```

`encrypt` generates `samples_local.key` on first run if it is missing. Losing the key means
losing the only way to decrypt the committed archive, so back it up wherever you keep
secrets. The decrypted contents must still respect each fixture's licence — the encryption
is a recovery/versioning convenience, **not** a redistribution licence.

## Manual decryption (without the script)

```bash
openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 \
  -pass file:samples_local.key -in samples_local.tar.gz.enc | tar xzf -
```
