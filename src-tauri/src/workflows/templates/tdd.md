# Test-Driven Development Workflow

## Overview
Red-Green-Refactor cycle for writing reliable, well-tested code.

## When to Use
Reference this workflow for TDD: `@~/.claude/workflows/tdd.md`

## The Cycle

### Phase 1: Red (Write a Failing Test)
1. Understand the requirement clearly
2. Write the smallest possible test that captures the requirement
3. Run the test — it MUST fail
4. If it passes, the test is wrong or the feature already exists

```
Rule: Never write production code without a failing test first
```

### Phase 2: Green (Make It Pass)
1. Write the minimum code to make the test pass
2. It's OK to hardcode, use shortcuts, or write "ugly" code
3. The only goal is a green test
4. Do NOT add extra functionality

```
Rule: Write the simplest code that makes the test pass — nothing more
```

### Phase 3: Refactor (Clean Up)
1. Remove duplication
2. Improve naming
3. Extract patterns
4. All tests must remain green after refactoring

```
Rule: Never refactor on red — all tests must pass before and after
```

## Guidelines

### Test Naming
```
test("[unit] [scenario] [expected result]")
// Example:
test("validateEmail rejects strings without @ symbol")
```

### Test Structure (AAA)
```
// Arrange — set up test data
// Act — execute the code under test
// Assert — verify the result
```

### What to Test
- Happy path (expected inputs)
- Edge cases (empty, null, boundary values)
- Error cases (invalid inputs, failures)
- State transitions

### What NOT to Test
- Private implementation details
- Framework/library internals
- Trivial getters/setters

## Iteration Strategy
1. Start with the simplest test case
2. Gradually add complexity
3. Each cycle should take 2-5 minutes
4. If a cycle takes longer, break the problem down further

## Anti-Patterns
- Writing tests after code (defeats the purpose)
- Writing multiple tests before any code
- Testing implementation instead of behavior
- Skipping the refactor phase
