# Database Migration Workflow

## Overview
Safe, reversible database schema change process.

## When to Use
Reference this workflow for DB changes: `@~/.claude/workflows/db-migration.md`

## Golden Rules
1. **Every migration must be reversible** (up + down)
2. **Never modify a deployed migration** — create a new one
3. **Test migrations on a copy of production data**
4. **Back up before migrating production**

## Process

### Step 1: Plan the Change
- What tables/columns are affected?
- Is it additive (safe) or destructive (risky)?
- What's the rollback strategy?
- Will it lock tables? For how long?

### Step 2: Categorize Risk

| Type | Risk | Examples |
|------|------|----------|
| Additive | Low | Add column, add table, add index |
| Modify | Medium | Rename column, change type, add constraint |
| Destructive | High | Drop column, drop table, remove constraint |

### Step 3: Write Migration

#### Safe Pattern: Add Column
```sql
-- Up
ALTER TABLE users ADD COLUMN avatar_url TEXT;

-- Down
ALTER TABLE users DROP COLUMN avatar_url;
```

#### Safe Pattern: Add Index (non-blocking)
```sql
-- Up
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);

-- Down
DROP INDEX idx_users_email;
```

#### Risky Pattern: Rename Column (multi-step)
```
Step 1: Add new column
Step 2: Backfill data
Step 3: Update application to use new column
Step 4: Drop old column (after verification period)
```

### Step 4: Test
- [ ] Run migration on empty database
- [ ] Run migration on copy of production data
- [ ] Run rollback and verify
- [ ] Test application with migrated schema
- [ ] Check migration timing on production-sized data

### Step 5: Deploy
1. Enable maintenance mode (if needed)
2. Create backup
3. Run migration
4. Verify schema
5. Test application
6. Disable maintenance mode

### Step 6: Monitor
- Watch for slow queries after index changes
- Monitor error rates after schema changes
- Keep rollback plan ready for 24-48 hours

## Migration Naming Convention
```
YYYY-MM-DD-HHMMSS_description.sql
2024-01-15-143000_add_avatar_to_users.sql
```

## Anti-Patterns
- Mixing data migration with schema migration
- Running untested migrations in production
- Dropping columns without deprecation period
- Large data backfills without batching
- Migrating during peak traffic
