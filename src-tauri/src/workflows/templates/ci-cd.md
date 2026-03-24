# CI/CD Pipeline Workflow

## Overview
Continuous Integration and Deployment best practices for reliable software delivery.

## When to Use
Reference this workflow for CI/CD setup: `@~/.claude/workflows/ci-cd.md`

## Pipeline Stages

### 1. Build
- Compile/transpile source code
- Install dependencies (with lockfile)
- Generate artifacts
- Cache dependencies between runs

### 2. Test
- Run unit tests
- Run integration tests
- Check code coverage (set minimum threshold)
- Run linting and formatting checks

### 3. Security
- Scan dependencies for vulnerabilities
- Run SAST (static analysis)
- Check for secrets in code
- Container image scanning (if applicable)

### 4. Deploy (Staging)
- Deploy to staging environment
- Run smoke tests
- Run end-to-end tests
- Performance benchmarks (optional)

### 5. Deploy (Production)
- Approval gate (manual or automated)
- Blue-green or canary deployment
- Health check verification
- Rollback on failure

## Branch Strategy

### Trunk-Based
```
main ──────────────────────── (always deployable)
  └── feature/xyz ──── PR ──── merge
```
- Short-lived feature branches (< 2 days)
- PR required for main
- Auto-deploy main to staging
- Manual promotion to production

### GitFlow (for versioned releases)
```
main ────────────────────── (production)
  └── develop ───────────── (staging)
       └── feature/xyz ──── (development)
```

## Configuration Checklist

### Build
- [ ] Reproducible builds (lockfile committed)
- [ ] Build caching enabled
- [ ] Build time < 5 minutes
- [ ] Artifacts stored and versioned

### Test
- [ ] Tests run in parallel where possible
- [ ] Test failures block merge
- [ ] Coverage reported (not just measured)
- [ ] Flaky tests tracked and fixed

### Deploy
- [ ] Zero-downtime deployments
- [ ] Automated rollback on health check failure
- [ ] Environment variables managed securely
- [ ] Database migrations run before app deploy

### Monitoring
- [ ] Deployment events tracked
- [ ] Error rate alerts post-deploy
- [ ] Performance regression alerts
- [ ] Deployment frequency measured

## Environment Promotion
```
Local → CI → Staging → Production
         ↓
     Run tests
     Security scan
     Build artifacts
```

## Rollback Strategy
1. **Instant**: Revert to previous deployment artifact
2. **Database**: Keep migrations backward-compatible
3. **Feature flags**: Disable new features without deploy
4. **Communication**: Notify team immediately on rollback

## Principles
- Automate everything that can be automated
- Fail fast — catch issues early in the pipeline
- Keep pipeline fast (< 10 min for basic checks)
- Make deployments boring and routine
