# Derived Requirements Template

Template for `.planning/REQUIREMENTS.derived.md` — planning-generated requirements derived from authoritative ones.

**RULES:**
- Planning commands MAY write to this file
- This file contains implementation-level requirements
- Never use this file as the source of truth

<template>

```markdown
# Derived Requirements: [Project Name]

**Generated:** [date]
**Source:** REQUIREMENTS.authoritative.md

## Implementation Requirements

These are derived from authoritative requirements during planning. They represent the breakdown needed to implement features.

### Authentication

- [ ] **DER-AUTH-01** (from AUTH-01): Validate email format, hash password with bcrypt, store in users table
- [ ] **DER-AUTH-02** (from AUTH-02): Generate verification token, send email via SMTP, create verification endpoint
- [ ] **DER-AUTH-03** (from AUTH-03): Generate reset token, password reset endpoint, validate token expiry

### [Category]

- [ ] **DER-[CAT]-01** (from [CAT]-01): [Implementation breakdown]

## Technical Requirements

Implementation-level requirements not directly from human requirements.

### Security

- [ ] **DER-SEC-01**: All endpoints require authentication middleware
- [ ] **DER-SEC-02**: Passwords hashed with bcrypt cost 12+

### Performance

- [ ] **DER-PERF-01**: API responses under 200ms (p95)
- [ ] **DER-PERF-02**: Database indexes on frequently queried columns

## Traceability

| Derived ID | Source | Status |
|------------|--------|--------|
| DER-AUTH-01 | AUTH-01 | Pending |
| DER-AUTH-02 | AUTH-02 | Pending |

---

*Generated: [date]*
*This file is auto-generated from authoritative requirements*
*Planning commands may update this file*
```

</template>

<guidelines>

**What goes here:**
- Implementation breakdowns of authoritative requirements
- Technical requirements needed to implement features
- Security, performance, and infrastructure requirements

**What does NOT go here:**
- Core feature definitions (those stay in authoritative)
- Anything a human didn't decide

**ID Format:** `DER-[CATEGORY]-[NUMBER]`

**Source Tracking:**
- Every derived requirement MUST cite its authoritative source
- Format: `(from AUTH-01)`

</guidelines>
