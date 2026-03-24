# Code Generation Workflow

## Overview
Standards and best practices for AI-assisted code generation.

## When to Use
Reference this workflow for AI code generation: `@~/.claude/workflows/code-generation.md`

## Principles

### 1. Context Is King
- Provide relevant file contents and patterns
- Reference existing code style and conventions
- Include type definitions and interfaces
- Specify the framework/library versions in use

### 2. Be Specific
```
Bad:  "Create a user component"
Good: "Create a React functional component UserCard that displays
       name, email, and avatar using the existing Card component
       from src/components/ui/Card.tsx. Follow the pattern in
       src/components/ProfileHeader.tsx."
```

### 3. Incremental Generation
- Generate one function/component at a time
- Verify each piece before generating the next
- Build up complexity gradually
- Don't generate entire files blindly

## Prompt Structure

### For New Code
```
Context: [what exists, what patterns to follow]
Task: [what to create, with specific requirements]
Constraints: [style, libraries, patterns to use/avoid]
Examples: [similar code in the project]
```

### For Modifications
```
Current code: [the existing code]
Problem: [what's wrong or what needs to change]
Expected: [desired behavior or output]
Constraints: [don't break X, maintain Y]
```

## Quality Checklist
After generating code, verify:
- [ ] Follows existing project patterns
- [ ] Uses project's existing utilities/helpers
- [ ] Type-safe (no `any`, proper generics)
- [ ] Error handling included
- [ ] No hardcoded values (use constants/config)
- [ ] No security vulnerabilities
- [ ] Tests included or test plan provided

## Anti-Patterns
- Generating code without reading existing patterns first
- Accepting generated code without review
- Generating too much at once (hard to verify)
- Ignoring project conventions for "cleaner" generated code
- Not providing enough context (garbage in, garbage out)

## Best Practices
1. **Study first**: Read 3 similar files before generating
2. **Small batches**: Generate, verify, iterate
3. **Trust but verify**: Always review generated code
4. **Preserve style**: Match existing formatting and patterns
5. **Test immediately**: Run tests after each generation
6. **Iterate**: Refine prompts based on output quality

## Template: Generation Request
```
I need to create [what].

Existing patterns to follow:
- [file1.ts]: [what pattern it demonstrates]
- [file2.ts]: [what pattern it demonstrates]

Requirements:
- [Requirement 1]
- [Requirement 2]

Constraints:
- Use [library/framework]
- Follow [pattern]
- Don't use [anti-pattern]
```
