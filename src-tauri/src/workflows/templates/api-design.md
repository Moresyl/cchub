# API Design Workflow

## Overview
RESTful API design patterns and best practices.

## When to Use
Reference this workflow for API design: `@~/.claude/workflows/api-design.md`

## Design Principles
1. **Consistency** — Same patterns everywhere
2. **Simplicity** — Easy to understand and use
3. **Predictability** — Behaves as expected
4. **Evolvability** — Can grow without breaking clients

## URL Design

### Resource Naming
```
GET    /api/v1/users          # List users
POST   /api/v1/users          # Create user
GET    /api/v1/users/:id      # Get user
PUT    /api/v1/users/:id      # Update user
DELETE /api/v1/users/:id      # Delete user
```

### Rules
- Use nouns, not verbs (`/users` not `/getUsers`)
- Use plural names (`/users` not `/user`)
- Use kebab-case for multi-word (`/user-profiles`)
- Nest for relationships (`/users/:id/orders`)
- Maximum 2 levels of nesting

## Request/Response Format

### Standard Response
```json
{
  "data": { ... },
  "meta": {
    "page": 1,
    "total": 100
  }
}
```

### Error Response
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Email is required",
    "details": [
      { "field": "email", "message": "must not be empty" }
    ]
  }
}
```

## HTTP Status Codes
| Code | Meaning | When to Use |
|------|---------|-------------|
| 200 | OK | Successful GET/PUT |
| 201 | Created | Successful POST |
| 204 | No Content | Successful DELETE |
| 400 | Bad Request | Invalid input |
| 401 | Unauthorized | Not authenticated |
| 403 | Forbidden | Not authorized |
| 404 | Not Found | Resource doesn't exist |
| 409 | Conflict | Duplicate / conflict |
| 422 | Unprocessable | Validation failed |
| 500 | Server Error | Unexpected failure |

## Pagination
```
GET /api/v1/users?page=2&limit=20

Response headers:
X-Total-Count: 100
Link: <...?page=3>; rel="next", <...?page=1>; rel="prev"
```

## Filtering and Sorting
```
GET /api/v1/users?status=active&role=admin
GET /api/v1/users?sort=-created_at,name
GET /api/v1/users?fields=id,name,email
```

## Versioning
- Use URL prefix: `/api/v1/`, `/api/v2/`
- Increment major version only for breaking changes
- Support previous version for deprecation period

## Authentication
- Use Bearer tokens in Authorization header
- Return 401 for missing/invalid tokens
- Return 403 for insufficient permissions
- Never put tokens in URLs

## Checklist
- [ ] RESTful URL structure
- [ ] Consistent response format
- [ ] Proper HTTP status codes
- [ ] Pagination for list endpoints
- [ ] Input validation with clear errors
- [ ] Authentication/authorization
- [ ] Rate limiting
- [ ] API versioning
- [ ] Documentation (OpenAPI/Swagger)
