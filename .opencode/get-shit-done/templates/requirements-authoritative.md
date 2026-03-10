# Authoritative Requirements Template

Template for `.planning/REQUIREMENTS.authoritative.md` — human-defined requirements that represent the source of truth.

**RULES:**
- Only humans write to this file
- This file is the authoritative source for planning
- Derived artifacts can reference these but never modify them

<template>

```markdown
# Authoritative Requirements: [Project Name]

**Defined:** [date]
**Core Value:** [from PROJECT.md]

## v1 Authoritative Requirements

These are the human-defined requirements. Each maps to roadmap phases.

### Authentication

- [ ] **AUTH-01**: User can sign up with email and password
- [ ] **AUTH-02**: User receives email verification after signup
- [ ] **AUTH-03**: User can reset password via email link
- [ ] **AUTH-04**: User session persists across browser refresh

### [Category 2]

- [ ] **[CAT]-01**: [Requirement description]
- [ ] **[CAT]-02**: [Requirement description]

## v2 Authoritative Requirements

Deferred to future release but still authoritative.

### [Category]

- **[CAT]-01**: [Requirement description]

## Out of Scope

Explicitly excluded from this project.

| Feature | Reason |
|---------|--------|
| [Feature] | [Why excluded] |

---

*Authoritative requirements defined: [date]*
*Only humans may modify this file*
```

</template>

<guidelines>

**What goes here:**
- Requirements written by humans (product owner, user)
- Core functional requirements that define the project
- Anything that "must be true" for the project to succeed

**What does NOT go here:**
- Implementation details (those go in derived)
- Technical architecture decisions
- Derived or inferred requirements from planning

**ID Format:** `[CATEGORY]-[NUMBER]` (AUTH-01, CONTENT-02)

**Versioning:**
- Each update creates a new version marker
- Previous versions preserved for audit trail

</guidelines>

<example>

```markdown
# Authoritative Requirements: CommunityApp

**Defined:** 2025-01-14
**Core Value:** Users can share and discuss content with people who share their interests

## v1 Authoritative Requirements

### Authentication

- [ ] **AUTH-01**: User can sign up with email and password
- [ ] **AUTH-02**: User receives email verification after signup
- [ ] **AUTH-03**: User can reset password via email link
- [ ] **AUTH-04**: User session persists across browser refresh

### Profiles

- [ ] **PROF-01**: User can create profile with display name
- [ ] **PROF-02**: User can upload avatar image

### Content

- [ ] **CONT-01**: User can create text post
- [ ] **CONT-02**: User can view feed of posts

## v2 Authoritative Requirements

### Notifications

- **NOTF-01**: User receives in-app notifications
- **NOTF-02**: User receives email for new followers

## Out of Scope

| Feature | Reason |
|---------|--------|
| Real-time chat | High complexity, not core to value |
| Video posts | Storage costs, defer to v2 |
| OAuth login | Email/password sufficient for v1 |

---

*Authoritative requirements defined: 2025-01-14*
*Only humans may modify this file*
```

</example>
