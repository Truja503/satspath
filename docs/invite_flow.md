# Invite Flow

Unknown receiver flow:

```txt
identifier -> no signed profile -> create invite -> receiver verifies email -> receiver wallet generates keys locally -> receiver publishes signed public profile -> sender re-resolves -> payment can proceed
```

Rules:

- Unknown receiver means invite only.
- No funds move before the receiver publishes a signed public payment profile.
- SatsPath does not generate seed phrases.
- SatsPath does not generate wallet private keys.
- SatsPath does not email private material.
- Email verification proves inbox access, not domain ownership.

Email content should say:

```txt
Someone wants to send you Bitcoin through SatsPath.
Click Receive to create or connect a wallet.
Your wallet will generate keys locally.
SatsPath will never see your seed or private keys.
```

Invite records store public routing metadata and identifier hashes. If raw email
is needed for delivery, it belongs only in the platform layer and should expire.
