<purpose>
Run reconciliation phase to compare implemented artifacts against authoritative requirements and identify cleanup/refactor work. This phase prohibits feature expansion and focuses on architectural hygiene.
</purpose>

<required_reading>
Read all files referenced by the invoking prompt's execution_context before starting.
</required_reading>

<process>

## 1. Initialize

Load project context:

```bash
INIT=$(node ./.opencode/get-shit-done/bin/gsd-tools.cjs init reconciliation)
```

Parse JSON for: `requirements_authoritative_path`, `roadmap_path`, `state_path`, `planning_exists`.

**If `planning_exists` is false:** Error — run `/gsd-new-project` first.

## 2. Check Preconditions

Verify at least one phase has been executed:

```bash
ls .planning/phases/*-VERIFICATION.md 2>/dev/null | wc -l
```

If zero, error: "Reconciliation requires at least one completed phase."

## 3. Load Authoritative Requirements

Read `.planning/REQUIREMENTS.authoritative.md`:
- Extract all v1 requirement IDs
- Note which are marked complete

## 4. Analyze Roadmap for Phase Status

Read `.planning/ROADMAP.md`:
- For each phase, note status (complete/in-progress/not-started)
- Note provenance ([A] or [D])

## 5. Analyze Implemented Phases

For each completed phase:
1. Read phase's VERIFICATION.md
2. Note which requirements were verified complete
3. Note any gaps between planned and verified

## 6. Identify Issues

### Dead Features
- Requirement was in roadmap but removed from authoritative requirements
- Implementation exists but no longer needed
- Check: Compare current REQUIREMENTS.authoritative.md to roadmap requirements

### Partial Implementations
- Requirement marked complete but implementation incomplete
- Tests missing or failing
- Check: Compare VERIFICATION.md results to requirement scope

### Over-abstracted Layers
- Abstraction layer created during planning but not used
- Interface/adapter pattern with single implementation
- Check: Look for files/classes with no references beyond their tests

### Orphaned Plans
- Plan exists but doesn't map to any requirement
- Derived phase with no clear authoritative source
- Check: Compare PLAN.md requirements field to REQUIREMENTS.authoritative.md

## 7. Generate Cleanup Tasks

Create `.planning/reconciliation/REPORT.md`:

```markdown
# Reconciliation Report

**Generated:** [date]

## Issues Found

### Dead Features
- [ ] [Feature/Code] - Was required by [REQ-ID] but requirement removed

### Partial Implementations
- [ ] [Feature] - Requirement [REQ-ID] marked complete but [gap]

### Over-abstracted Layers
- [ ] [File/Layer] - Abstraction with single implementation

### Orphaned Plans
- [ ] [Phase/Plan] - No authoritative requirement source

## Cleanup Tasks

Generate deletion/refactor tasks for each issue:
- [ ] Delete unused [file/component]
- [ ] Remove unnecessary [abstraction]
- [ ] Complete partial [feature]

## Feature Expansion Prohibited

This reconciliation phase does NOT include:
- New features
- New requirements
- Scope expansion

Next phase should address cleanup before new work.
```

## 8. Present Results

Display summary to user:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 GSD ► RECONCILIATION RESULTS
 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Dead Features:** N
**Partial Implementations:** N
**Over-abstracted Layers:** N
**Orphaned Plans:** N

Total cleanup tasks: N

[Show list of issues]
```

## 9. Request Action

Ask user what to do:

```
Use question:
- header: "Cleanup"
- question: "How to proceed with reconciliation results?"
- options:
  - "Generate cleanup tasks" — Create PLAN.md with deletion/refactor tasks
  - "Review details first" — Open REPORT.md for full analysis
  - "Skip cleanup" — Proceed to next phase without cleanup
```

## 10. Handle User Choice

**If "Generate cleanup tasks":**
- Create cleanup PLAN.md in .planning/reconciliation/
- Include all identified issues as tasks
- Mark as phase for execution

**If "Review details first":**
- Open REPORT.md in editor
- Return to step 9 after review

**If "Skip cleanup":**
- Note reconciliation was run but skipped
- Allow proceeding to next phase

## 11. Prohibit Feature Expansion

CRITICAL: This phase MUST NOT include:
- New requirements from user
- New features not in authoritative requirements
- Scope creep

If user requests new features:
```
This is a reconciliation phase — cleanup only. 
New features require updating REQUIREMENTS.authoritative.md first.
```

</process>

<output>

- `.planning/reconciliation/REPORT.md` — detailed analysis
- `.planning/reconciliation/XX-CLEANUP-PLAN.md` (if cleanup tasks generated)

</output>

<success_criteria>

- [ ] Authoritative requirements loaded
- [ ] Roadmap phases analyzed
- [ ] Completed phases verified
- [ ] Dead features identified
- [ ] Partial implementations identified
- [ ] Over-abstracted layers identified
- [ ] Orphaned plans identified
- [ ] REPORT.md generated
- [ ] Cleanup tasks created (if requested)
- [ ] Feature expansion prohibited
- [ ] User knows next step

</success_criteria>
