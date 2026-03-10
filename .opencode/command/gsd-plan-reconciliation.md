---
description: Run reconciliation phase to compare artifacts to requirements and identify cleanup tasks
argument-hint: "[--auto]"
tools:
  read: true
  write: true
  bash: true
  glob: true
  grep: true
  task: true
---

<objective>
Run a reconciliation phase that compares implemented artifacts to authoritative requirements and generates cleanup/refactor tasks.

**When invoked, must:**
1. Compare implemented artifacts to authoritative requirements
2. Identify:
   - Dead features (requirement removed but implementation exists)
   - Partial implementations (requirement changed but code incomplete)
   - Over-abstracted layers (unnecessary abstraction from planning)
   - Orphaned plans (plans not tied to requirements)
3. Generate deletion/refactor tasks
4. Prohibit feature expansion

This phase must be selectable before starting new phases.
</objective>

<context>
**Flags:**
- `--auto` — Skip confirmation, run reconciliation automatically

**Preconditions:**
- `.planning/REQUIREMENTS.authoritative.md` must exist
- `.planning/ROADMAP.md` must exist
- At least one phase must be complete
</context>

<execution_context>
@./.opencode/get-shit-done/workflows/reconciliation-phase.md
</execution_context>

<process>

Execute reconciliation-phase.md workflow end-to-end.

</process>

<success_criteria>
- [ ] Authoritative requirements compared to implementation
- [ ] Dead features identified
- [ ] Partial implementations identified
- [ ] Orphaned plans identified
- [ ] Cleanup tasks generated
- [ ] User can approve cleanup tasks
- [ ] Feature expansion prohibited in this phase
</success_criteria>
