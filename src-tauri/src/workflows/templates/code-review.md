# Code Review Workflow

## Overview
Systematic code review process to ensure code quality, consistency, and correctness.

## When to Use
Reference this workflow when reviewing pull requests or code changes: `@~/.claude/workflows/code-review.md`

## Review Checklist

### 1. Correctness
- Does the code do what it claims to do?
- Are edge cases handled?
- Are error conditions handled gracefully?
- Is input validated at system boundaries?

### 2. Readability
- Are variable/function names clear and descriptive?
- Is the code self-documenting without excessive comments?
- Is the control flow easy to follow?
- Are complex sections adequately commented?

### 3. Architecture
- Does it follow existing patterns in the codebase?
- Is the responsibility clearly defined (single responsibility)?
- Are dependencies appropriate and minimal?
- Is the abstraction level consistent?

### 4. Performance
- Are there obvious performance issues (N+1 queries, unnecessary loops)?
- Is memory usage reasonable?
- Are expensive operations cached where appropriate?

### 5. Security
- Is user input sanitized?
- Are SQL queries parameterized?
- Are secrets properly managed (not hardcoded)?
- Is authentication/authorization checked where needed?

### 6. Testing
- Are there tests for the new/changed code?
- Do tests cover edge cases?
- Are tests readable and maintainable?
- Do existing tests still pass?

## Review Response Template

```
## Summary
[One-line summary of the change]

## Findings
### Critical
- [ ] [Issue description + file:line]

### Suggestions
- [ ] [Improvement suggestion]

### Positive
- [What was done well]
```

## Principles
- Be specific: reference file and line numbers
- Suggest solutions, not just problems
- Distinguish between blocking issues and suggestions
- Acknowledge good patterns when you see them
