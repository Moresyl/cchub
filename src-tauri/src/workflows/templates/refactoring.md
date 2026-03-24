# Refactoring Guide

## Overview
Safe, incremental code restructuring without changing external behavior.

## When to Use
Reference this workflow for refactoring: `@~/.claude/workflows/refactoring.md`

## Golden Rules
1. **Never refactor and change behavior at the same time**
2. **All tests must pass before AND after each step**
3. **Make small, reversible changes**
4. **Commit after each successful step**

## Before Starting
- [ ] All tests pass
- [ ] You understand what the code does
- [ ] You have a clear goal for the refactoring
- [ ] You've communicated the plan (if team)

## Common Refactoring Patterns

### Extract Function
When: A code block does one identifiable thing
```
Before: 50-line function with inline logic
After: 5 small functions with clear names
```

### Rename for Clarity
When: Names don't communicate intent
```
Before: const d = getData(x)
After: const userProfile = fetchUserProfile(userId)
```

### Remove Duplication
When: Same logic appears in 3+ places
```
Before: Copy-pasted validation in 4 handlers
After: Shared validateInput() utility
```

### Simplify Conditionals
When: Nested if/else is hard to follow
```
Before: if (a) { if (b) { if (c) { ... } } }
After: Early returns + guard clauses
```

### Replace Magic Values
When: Hardcoded numbers/strings without context
```
Before: if (status === 3) ...
After: if (status === OrderStatus.SHIPPED) ...
```

### Flatten Hierarchy
When: Deep nesting makes code hard to read
- Use early returns
- Extract helper functions
- Use array methods instead of nested loops

## Step-by-Step Process
1. Identify the smell (what's wrong)
2. Choose the refactoring technique
3. Write a characterization test if missing
4. Apply the refactoring in small steps
5. Run tests after each step
6. Commit working state

## When NOT to Refactor
- Under time pressure with no tests
- Code that will be deleted soon
- Code you don't understand yet
- During a feature implementation (do it separately)
