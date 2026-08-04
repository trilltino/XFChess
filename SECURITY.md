# Security Policy

Security reviews and pentesting of XFChess are welcome. If you believe you've found
a security issue, please notify us so we can resolve it promptly.

## Reporting Vulnerabilities

Please report security issues to:
- Email: security@xfchess.org
- GitHub Security Advisory: [Report a vulnerability](https://github.com/trilltino/XFChess/security/advisories/new)

Vulnerabilities are relevant even when they are not directly exploitable, for example XSS mitigated by CSP.

## Scope

This security policy applies to all of XFChess's source repositories and infrastructure, including:

- **Web Frontend**: Web-based chess interface
- **Game Client**: Desktop application (Tauri)
- **Backend API**: REST API for game management
- **Solana Program**: Smart contract for move verification and tournaments
- **Infrastructure**: Hosting and deployment infrastructure

## Contract Security

The XFChess Solana program handles:
- Game move verification
- Tournament management
- Prize escrow and distribution
- Ephemeral Rollups delegation

### Contract-Specific Vulnerabilities

Please report issues related to:
- Unauthorized fund extraction from prize escrows
- Move verification bypass
- Tournament manipulation
- Delegation record tampering
- Reentrancy attacks
- Integer overflow/underflow in prize calculations
- Unauthorized account modifications

### Contract Resolution Options

When contract vulnerabilities are reported, we will:
1. **Assess severity** and potential impact on funds
2. **Pause program** if necessary (using Solana's upgrade authority)
3. **Deploy fix** after thorough testing on devnet
4. **Upgrade program** on mainnet
5. **Compensate affected users** if funds were lost (community fund available)
6. **Full disclosure** after resolution

## Rules for Testing Production Infrastructure

- Perform testing only on assets that are in scope
- Make good faith efforts to avoid privacy violations, destruction of data, interruption or degradation of service, and any annoyance or inconvenience to XFChess users, including spam
- If a vulnerability provides unintended access to data, limit the amount of data you access to the minimum required for effectively demonstrating a Proof of Concept
- Do not create more than 5 user accounts for testing
- All forms of social engineering (e.g., phishing) are strictly prohibited
- Respect HTTP rate limits, i.e., slow down when you receive HTTP 429
- **Do not test contract vulnerabilities on mainnet** - use devnet only

## Exclusions

Please do not submit issues regarding:

- Theoretical vulnerabilities without any proof or demonstration of the real presence of the vulnerability
- Findings from automated tools without providing a Proof of Concept
- (D)DoS
- Missing X-Content-Type-Options, Referrer-Policy or Feature-Policy headers
- Non-sensitive data disclosure, including software version information, confirmation that a specific email address is in use, confirming the existence (but not content) of sensitive information
- Content spoofing and text injection issues without showing an attack vector/without being able to modify HTML/CSS
- Previously known vulnerable software or libraries without a working Proof of Concept
- Vulnerabilities requiring access to a user's browser, or a smartphone, or email account
- CSRF from local files (file://)
- Cheating at puzzles or training modes (these are for practice, not competition)

## Response Targets

We aim to meet the following response targets:

- **Time to first response**: 2 days after report submission
- **Time to resolution**: 30 days

## Disclosure

All vulnerabilities will be disclosed via GitHub Security Advisory once they have been confirmed and resolved.

## Rewards

We do not currently pay cash bounties. Contributors are acknowledged in our security advisories.

## Safe Harbor

Any activities conducted in a manner consistent with this policy will be considered authorized conduct (even without prior coordination) and we will not initiate legal action against you. If legal action is initiated by a third party against you in connection with activities conducted under this policy, we will take steps to make it known that your actions were conducted in compliance with this policy.

## Best Practices

### For Users
- Keep your wallet private keys secure
- Use hardware wallets for significant holdings
- Only interact with official XFChess contracts
- Report suspicious activity

### For Developers
- Follow secure coding practices
- Use dependency scanning tools
- Test contract changes thoroughly on devnet
- Enable security features in production

Thank you for helping keep XFChess users safe!
