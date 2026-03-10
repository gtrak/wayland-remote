---
description: Regenerate roadmap from authoritative requirements, preserving existing implementation progress
argument-hint: "[--force]"
agent: gsd-roadmapper
tools:
  read: true
  write: true
  bash: true
  glob: true
  grep: true
  task: true
---

<objective>
Regenerate roadmap from authoritative requirements while preserving existing implementation progress.

**Behavior:**
1. Analyze existing phases for completed work
2. Map legacy phases to authoritative requirements they satisfy
3. Generate new roadmap with legacy status preserved
4. Require confirmation before overwrite

**When to use:**
- Roadmap has accumulated too many derived/planning phases
- Architectural drift needs pruning
- Want to reset to human-defined requirements while keeping implementation history
</objective>

<context>
**Flags:**
- `--force` — Skip confirmation prompt (for automation)

**Preconditions:**
- `.planning/REQUIREMENTS.authoritative.md` must exist
- `.planning/ROADMAP.md` must exist
</context>

<execution_context>
@./.opencode/get-shit-done/agents/gsd-roadmapper.md
@./.opencode/get-shit-done/templates/roadmap.md
</execution_context>

<process>

## 1. Validate Preconditions

Check that required files exist:

```bash
ls .planning/REQUIREMENTS.authoritative.md .planning/ROADMAP.md
```

If either missing, error with guidance.

## 2. Analyze Current Roadmap

Read current ROADMAP.md and identify:
- All phases with provenance [A] (authoritative-driven)
- All phases with provenance [D] (derived)
- Current phase ordering and dependencies

Count phases by provenance type:
```
Authoritative phases: N
Derived phases: M
```

## 3. Map Existing Implementation

Scan `.planning/phases/` for completed work:

```bash
# Find all phases with summaries (completed work)
for dir in .planning/phases/*/; do
  phase=$(basename "$dir")
  summary_count=$(ls "$dir"*-SUMMARY.md 2>/dev/null | wc -l)
  plan_count=$(ls "$dir"*-PLAN.md 2>/dev/null | wc -l)
  if [ "$summary_count" -gt 0 ]; then
    echo "$phase: COMPLETE ($summary_count/$plan_count plans)"
  fi
done
```

For each completed phase:
1. Read the phase's SUMMARY.md to understand what was implemented
2. Read the original roadmap phase entry to see what requirements it addressed
3. Map to authoritative requirements from REQUIREMENTS.authoritative.md

Output mapping:
```
=== EXISTING IMPLEMENTATION ===

Phase 01-foundation: COMPLETE (5/5 plans)
  → Satisfies: PROXY-01, PROXY-02, PROXY-03

Phase 02-core-proxying: COMPLETE (9/9 plans)
  → Satisfies: AUTH-01, AUTH-02, AUTH-03

[Continue for all completed phases]
```

## 4. Extract Authoritative Requirements

Read REQUIREMENTS.authoritative.md and extract:
- All v1 requirement IDs
- Categories and groupings
- Which requirements are already satisfied by legacy phases

## 5. Generate Diff with Legacy Preservation

Compare current roadmap to authoritative-only version:

Output:
```
=== ROADMAP DIFF ===

EXISTING IMPLEMENTATION (preserved):
Phase 01-foundation → Complete → satisfies PROXY-01, PROXY-02
Phase 02-core-proxying → Complete → satisfies AUTH-01, AUTH-02, AUTH-03
...

DERIVED PHASES (removed):
- Phase X: [name] [reason]
- Phase Y: [name] [reason]

REMAINING (not yet implemented):
- Phase 3: [name] (from authoritative requirements)
- Phase 4: [name] (from authoritative requirements)

REQUIREMENT COVERAGE:
| Requirement | Status | Legacy Phase |
|-------------|--------|--------------|
| AUTH-01 | ✓ Complete | 02-core-proxying |
| AUTH-02 | ✓ Complete | 02-core-proxying |
| PROXY-01 | ✓ Complete | 01-foundation |
| PROXY-02 | ✓ Complete | 01-foundation |
| CONTENT-01 | Pending | - |
```

## 6. Request Confirmation

If `--force` not set, ask user:

```
Use question:
- header: "Regenerate Roadmap"
- question: "This will regenerate the roadmap from authoritative requirements while preserving 5 completed phases. Derived phases will be removed. Continue?"
- options:
  - "Regenerate" — Create new roadmap with legacy status
  - "View mapping" — See detailed legacy mapping first
  - "Cancel" — Keep current roadmap
```

## 7. Generate New Roadmap

If confirmed:

Call gsd-roadmapper agent with:
- REQUIREMENTS.authoritative.md (authoritative requirements)
- REQUIREMENTS.derived.md (derived requirements - will be cleared)
- Legacy mapping data from step 3
- Instruction to create roadmap with:
  - All phases tagged [A]
  - Legacy status section showing completed phases
  - DerivedFrom citing authoritative requirement IDs

Agent creates new ROADMAP.md with structure:

```markdown
# Roadmap vX.X: [Project Name]

## Legacy Implementation Status

These requirements are already satisfied by previous implementation:

| Phase | Status | Satisfies Requirements |
|-------|--------|------------------------|
| 01-foundation | ✓ Complete | PROXY-01, PROXY-02, PROXY-03 |
| 02-core-proxying | ✓ Complete | AUTH-01, AUTH-02, AUTH-03 |

## Phases

### Phase 1: Foundation
**Provenance**: [A] — Authoritative-driven (Legacy)
**DerivedFrom**: PROXY-01, PROXY-02, PROXY-03
**Status**: Complete (5/5 plans)
**Legacy Phase**: 01-foundation

### Phase 2: Core Proxying
**Provenance**: [A] — Authoritative-driven (Legacy)
**DerivedFrom**: AUTH-01, AUTH-02, AUTH-03
**Status**: Complete (9/9 plans)
**Legacy Phase**: 02-core-proxying

### Phase 3: [New Phase]
**Provenance**: [A] — Authoritative-driven
**DerivedFrom**: CONTENT-01, CONTENT-02
**Status**: Not Started
**Plans**: 0/? plans
```

## 7.5. Validate Phase Directories

After generating the new roadmap, validate that phase directories match:

```bash
# Check each legacy phase has correct directory
for legacy_phase in 01-foundation 02-core-proxying; do
  new_dir=$(ls -d .planning/phases/*-${legacy_phase} 2>/dev/null | head -1)
  if [ -z "$new_dir" ]; then
    echo "WARNING: Phase directory for $legacy_phase not found"
  fi
done

# Check for orphan directories (in new roadmap but not on disk)
echo "Orphan directories (no corresponding phase in new roadmap):"
ls -d .planning/phases/*/ 2>/dev/null | while read dir; do
  phase_name=$(basename "$dir" | sed 's/^[0-9]*-//')
  if ! grep -q "### Phase.*$phase_name" .planning/ROADMAP.md; then
    echo "  - $(basename $dir)"
  fi
done
```

If phase directories don't match the new roadmap naming, offer to rename:
- Old: `01-foundation` → New: `01-foundation` (same, no change)
- If renamed: offer `mv` command to user

## 8. Commit Changes

```bash
node ./.opencode/get-shit-done/bin/gsd-tools.cjs commit "refactor: regenerate roadmap from authoritative requirements (preserved 5 completed phases)" --files .planning/ROADMAP.md .planning/STATE.md .planning/REQUIREMENTS.derived.md
```

Note: REQUIREMENTS.derived.md is cleared since we're regenerating from authoritative only.

## 9. Present Results

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 GSD ► ROADMAP REGENERATED ✓
 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Preserved:** 5 completed phases (legacy status)
**New phases:** 4 remaining requirements to implement
**Removed:** M derived phases

Requirement Coverage:
- Complete: 8 requirements (from legacy phases)
- Pending: 4 requirements

Next: /gsd-plan-phase [next-unstarted-phase]
```

</process>

<success_criteria>
- [ ] Current roadmap analyzed for provenance types
- [ ] Existing implementation mapped (completed phases → requirements)
- [ ] Diff generated showing legacy preservation
- [ ] User confirmed (or --force used)
- [ ] New roadmap generated with legacy status section
- [ ] All completed phases preserved with requirement mappings
- [ ] Changes committed
- [ ] User sees summary and next step
</success_criteria>
