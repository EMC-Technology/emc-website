# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅ |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub Issues.**

If you discover a security vulnerability in this project, please report it responsibly:

1. **Email**: Send a detailed report to the project maintainers via GitHub's private vulnerability reporting feature
2. **GitHub Security Advisory**: Use [GitHub's private vulnerability reporting](https://github.com/emc-technology/knowledge-system/security/advisories/new) to submit your findings

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if available)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 5 business days
- **Fix Timeline**: Depends on severity — Critical within 7 days, High within 14 days, Medium within 30 days

### Security Architecture

This project implements a zero-trust security model:

- **Authentication**: JWT + Argon2id password hashing
- **Authorization**: Cedar-style ABAC (Attribute-Based Access Control) with Deny-Override
- **Data Protection**: AES-256-GCM encryption, BLAKE3 hashing, PII scanning (11 patterns)
- **Audit Trail**: CQRS + Event Sourcing provides complete audit chain
- **Supply Chain**: cargo-deny enforces license compliance and vulnerability scanning

## Security Features

| Feature | Implementation |
|---------|---------------|
| Error Handling | error-core unified error model with structured classification |
| PII Detection | 11 regex patterns + 5 masking strategies + JSON recursive scanning |
| Encryption | AES-256-GCM + BLAKE3 + key management (cross-platform secure storage) |
| Access Control | ABAC policy engine (Cedar-style) + LRU cache + batch evaluation + hot reload |
| Audit Logging | CQRS Event Sourcing + HMAC-signed event chain |
| Rate Limiting | Token bucket algorithm |
