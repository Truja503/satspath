# Release Readiness Checklist for SatsPath v0

## Security and Privacy
- [x] All cryptographic implementations reviewed.
- [x] No private material (seeds, keys) included in profiles or payment pointers.
- [x] Sensitive user data masked in preview modes.

## Functional Testing
- [x] All unit tests pass across workspaces.
- [x] Integration tests for HTTP resolver and LNURL pass.
- [x] Testnet execution paths enabled; mainnet execution paths strictly disabled.

## Code Quality
- [x] CI workflows in place for formatting, linting, and testing.
- [x] PR template and branch protection guidelines established.

## Documentation
- [x] README reflects experimental nature of swap engine.
- [x] Core architecture and protocol concepts fully documented.
