# Security Audit Workflow

## Overview
Security review checklist based on OWASP Top 10 and common vulnerability patterns.

## When to Use
Reference this workflow for security reviews: `@~/.claude/workflows/security-audit.md`

## OWASP Top 10 Checklist

### 1. Injection (SQL, Command, LDAP)
- [ ] All SQL queries use parameterized statements
- [ ] Shell commands don't include user input
- [ ] LDAP/XPath queries are properly escaped
- [ ] ORM queries avoid raw string concatenation

### 2. Broken Authentication
- [ ] Passwords are hashed with bcrypt/argon2 (not MD5/SHA)
- [ ] Session tokens are sufficiently random
- [ ] Login has rate limiting / account lockout
- [ ] Password reset tokens expire

### 3. Sensitive Data Exposure
- [ ] Secrets are in env vars, not source code
- [ ] HTTPS enforced for all endpoints
- [ ] Sensitive fields excluded from logs
- [ ] API responses don't leak internal details

### 4. XML External Entities (XXE)
- [ ] XML parsers disable external entity processing
- [ ] Prefer JSON over XML where possible

### 5. Broken Access Control
- [ ] Authorization checked on every protected endpoint
- [ ] Users can only access their own resources
- [ ] Admin functions require proper role checks
- [ ] CORS configured correctly

### 6. Security Misconfiguration
- [ ] Debug mode disabled in production
- [ ] Default credentials changed
- [ ] Error pages don't reveal stack traces
- [ ] Unnecessary services/ports disabled

### 7. Cross-Site Scripting (XSS)
- [ ] All user input is escaped before rendering
- [ ] Content-Security-Policy headers set
- [ ] innerHTML / dangerouslySetInnerHTML avoided
- [ ] URL parameters sanitized

### 8. Insecure Deserialization
- [ ] Don't deserialize untrusted data
- [ ] Validate/whitelist deserialized types
- [ ] Sign serialized objects if needed

### 9. Using Components with Known Vulnerabilities
- [ ] Dependencies are up to date
- [ ] No known CVEs in dependency tree
- [ ] Automated vulnerability scanning in CI

### 10. Insufficient Logging & Monitoring
- [ ] Authentication events are logged
- [ ] Failed access attempts are tracked
- [ ] Logs don't contain sensitive data
- [ ] Alerting configured for anomalies

## Quick Scan Process
1. Check authentication and authorization flows
2. Search for raw SQL / command execution
3. Review input handling at API boundaries
4. Check dependency vulnerability reports
5. Review error handling (no info leaks)
6. Verify secret management

## Reporting Format
```
## [SEVERITY] Finding Title
- **Location**: file:line
- **Risk**: What could an attacker do?
- **Remediation**: How to fix it
- **Priority**: Critical / High / Medium / Low
```
