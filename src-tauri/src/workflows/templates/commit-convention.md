# Commit Convention

## Overview
Standardized Git commit message format for clear project history.

## When to Use
Reference this workflow for commits: `@~/.claude/workflows/commit-convention.md`

## Format
```
<type>: <subject>

[optional body]

[optional footer]
```

## Types
| Type | When to Use |
|------|-------------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `refactor` | Code restructuring (no behavior change) |
| `docs` | Documentation only changes |
| `test` | Adding or updating tests |
| `chore` | Build, tooling, dependencies |
| `perf` | Performance improvement |
| `style` | Formatting, whitespace (no logic change) |
| `ci` | CI/CD pipeline changes |

## Subject Rules
- Use imperative mood: "add feature" not "added feature"
- Don't capitalize the first letter
- No period at the end
- Keep under 50 characters
- Describe WHAT changed, not HOW

## Body Rules
- Separate from subject with a blank line
- Explain WHY the change was made
- Wrap at 72 characters
- Use bullet points for multiple points

## Examples

### Good
```
feat: add email validation to signup form

Prevents invalid emails from being submitted. Uses RFC 5322
regex pattern for validation.
```

```
fix: resolve race condition in order processing

Orders were occasionally duplicated when users double-clicked
the submit button. Added debounce and server-side idempotency.
```

### Bad
```
updated stuff
fixed bug
WIP
misc changes
```

## Breaking Changes
```
feat: change authentication to JWT

BREAKING CHANGE: Session-based auth is removed.
All clients must send Bearer token in Authorization header.
```

## Tips
- Each commit should represent one logical change
- If you need "and" in the subject, split into two commits
- Commit early and often
- Don't mix refactoring with feature changes
