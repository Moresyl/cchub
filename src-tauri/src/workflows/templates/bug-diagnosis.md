# Bug Diagnosis Workflow

## Overview
Systematic approach to identifying, isolating, and fixing bugs.

## When to Use
Reference this workflow when debugging: `@~/.claude/workflows/bug-diagnosis.md`

## Step 1: Reproduce
- Get exact steps to reproduce the bug
- Identify the expected vs actual behavior
- Determine if it's consistent or intermittent
- Note the environment (OS, browser, version, config)

```
Reproducible bug = 80% solved
```

## Step 2: Isolate
- Find the minimal reproduction case
- Remove unrelated code/config until the bug persists
- Binary search: comment out half the code, see if bug remains
- Check if the bug exists on a clean branch/environment

## Step 3: Understand
- Read the code around the failure point
- Trace the data flow from input to output
- Check recent changes (git log, git blame)
- Look for similar past bugs in commit history

## Step 4: Hypothesize
- Form 2-3 hypotheses about the root cause
- Rank them by likelihood
- Design a test for the most likely hypothesis

```
Bad: "Something is wrong with the database"
Good: "The user_id foreign key is NULL because the insert happens before the user record is committed"
```

## Step 5: Verify
- Add targeted logging/breakpoints
- Write a failing test that demonstrates the bug
- Confirm the hypothesis with evidence

## Step 6: Fix
- Fix the root cause, not the symptom
- Keep the fix minimal and focused
- Ensure the failing test now passes
- Check for similar issues elsewhere in the codebase

## Step 7: Validate
- Run the full test suite
- Verify the original reproduction steps no longer trigger the bug
- Check for regressions
- Document the root cause in the commit message

## Debugging Techniques

### Divide and Conquer
- Binary search through code paths
- Comment out sections to narrow scope
- Use git bisect for regression bugs

### Trace and Log
- Add temporary logging at key points
- Log input/output of suspected functions
- Check network requests and responses

### Compare and Contrast
- Does it work in a different environment?
- Does it work with different data?
- Did it work in a previous version?

## Common Root Causes
- Race conditions / timing issues
- Off-by-one errors
- Null/undefined references
- Stale cache or state
- Encoding/charset mismatches
- Environment differences (dev vs prod)
