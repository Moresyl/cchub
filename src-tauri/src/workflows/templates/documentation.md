# Documentation Workflow

## Overview
Standards for generating and maintaining project documentation.

## When to Use
Reference this workflow for documentation: `@~/.claude/workflows/documentation.md`

## Documentation Layers

### 1. Code-Level (Inline)
- Function/method doc comments for public APIs
- Complex algorithm explanations
- TODO/FIXME with context
- NOT: obvious code that explains itself

### 2. Module-Level (README)
- Purpose of the module
- How to use it (quick example)
- Dependencies and requirements
- Architecture decisions

### 3. Project-Level (Docs)
- Getting started guide
- Architecture overview
- API reference
- Deployment guide
- Contributing guidelines

## Writing Guidelines

### Be Concise
```
Bad:  "This function is used to validate the email address of the user"
Good: "Validates email against RFC 5322"
```

### Show, Don't Just Tell
Always include code examples:
```
// Usage:
const result = validate(input, { strict: true });
```

### Keep It Current
- Update docs when code changes
- Delete outdated documentation
- Date-stamp architecture decisions

## API Documentation Template
```markdown
## endpoint_name

Brief description of what this endpoint does.

**Method**: GET/POST/PUT/DELETE
**Path**: `/api/v1/resource`
**Auth**: Required / Public

### Parameters
| Name | Type | Required | Description |
|------|------|----------|-------------|
| id   | string | yes    | Resource ID |

### Response
​```json
{
  "status": "success",
  "data": { ... }
}
​```

### Errors
| Code | Description |
|------|-------------|
| 404  | Resource not found |
```

## README Template
```markdown
# Project Name

One-line description.

## Quick Start
​```bash
npm install
npm run dev
​```

## Architecture
[Brief overview + diagram link]

## Development
[Build, test, lint commands]

## Deployment
[How to deploy]
```

## Principles
- Documentation is a product — treat it with care
- Write for your future self (you in 6 months)
- Prefer examples over prose
- Keep a changelog for users
