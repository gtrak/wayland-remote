/**
 * Milestone — Milestone and requirements lifecycle operations
 */

const fs = require('fs');
const path = require('path');
const { output, error } = require('./core.cjs');
const { extractFrontmatter } = require('./frontmatter.cjs');

function cmdRequirementsMarkComplete(cwd, reqIdsRaw, raw) {
  if (!reqIdsRaw || reqIdsRaw.length === 0) {
    error('requirement IDs required. Usage: requirements mark-complete REQ-01,REQ-02 or REQ-01 REQ-02');
  }

  // Accept comma-separated, space-separated, or bracket-wrapped: [REQ-01, REQ-02]
  const reqIds = reqIdsRaw
    .join(' ')
    .replace(/[\[\]]/g, '')
    .split(/[,\s]+/)
    .map(r => r.trim())
    .filter(Boolean);

  if (reqIds.length === 0) {
    error('no valid requirement IDs found');
  }

  // Support both legacy REQUIREMENTS.md and new separated files
  const reqAuthPath = path.join(cwd, '.planning', 'REQUIREMENTS.authoritative.md');
  const reqDerivedPath = path.join(cwd, '.planning', 'REQUIREMENTS.derived.md');
  const reqLegacyPath = path.join(cwd, '.planning', 'REQUIREMENTS.md');

  // Determine which files exist
  const authExists = fs.existsSync(reqAuthPath);
  const derivedExists = fs.existsSync(reqDerivedPath);
  const legacyExists = fs.existsSync(reqLegacyPath);

  let updated = [];
  let notFound = [];

  // Function to process a requirements file
  function processRequirementsFile(reqPath, reqIds, isDerived) {
    if (!fs.existsSync(reqPath)) return { updated: [], notFound: [] };

    let reqContent = fs.readFileSync(reqPath, 'utf-8');
    const fileUpdated = [];
    const fileNotFound = [];

    for (const reqId of reqIds) {
      let found = false;

      // For derived requirements, also check DER- prefix
      if (isDerived) {
        const derivedId = reqId.startsWith('DER-') ? reqId : `DER-${reqId}`;
        // Update checkbox: - [ ] **DER-REQ-ID** → - [x] **DER-REQ-ID**
        const checkboxPattern = new RegExp(`(-\\s*\\[)[ ](\\]\\s*\\*\\*${derivedId}\\*\\*)`, 'gi');
        if (checkboxPattern.test(reqContent)) {
          reqContent = reqContent.replace(checkboxPattern, '$1x$2');
          found = true;
        }
      } else {
        // Update checkbox: - [ ] **REQ-ID** → - [x] **REQ-ID**
        const checkboxPattern = new RegExp(`(-\\s*\\[)[ ](\\]\\s*\\*\\*${reqId}\\*\\*)`, 'gi');
        if (checkboxPattern.test(reqContent)) {
          reqContent = reqContent.replace(checkboxPattern, '$1x$2');
          found = true;
        }
      }

      // Update traceability table: | REQ-ID | Phase N | Pending | → | REQ-ID | Phase N | Complete |
      const tablePattern = new RegExp(`(\\|\\s*${reqId}\\s*\\|[^|]+\\|)\\s*Pending\\s*(\\|)`, 'gi');
      if (tablePattern.test(reqContent)) {
        // Re-read since test() advances lastIndex for global regex
        reqContent = reqContent.replace(
          new RegExp(`(\\|\\s*${reqId}\\s*\\|[^|]+\\|)\\s*Pending\\s*(\\|)`, 'gi'),
          '$1 Complete $2'
        );
        found = true;
      }

      if (found) {
        fileUpdated.push(reqId);
      } else {
        fileNotFound.push(reqId);
      }
    }

    if (fileUpdated.length > 0) {
      fs.writeFileSync(reqPath, reqContent, 'utf-8');
    }

    return { updated: fileUpdated, notFound: fileNotFound };
  }

  // Process authoritative requirements
  if (authExists) {
    const authResult = processRequirementsFile(reqAuthPath, reqIds, false);
    updated = updated.concat(authResult.updated);
    notFound = notFound.concat(authResult.notFound);
  }

  // Process derived requirements
  if (derivedExists) {
    const derivedResult = processRequirementsFile(reqDerivedPath, reqIds, true);
    updated = updated.concat(derivedResult.updated);
    notFound = notFound.concat(derivedResult.notFound);
  }

  // Fallback to legacy file if neither new file exists
  if (!authExists && !derivedExists && legacyExists) {
    const legacyResult = processRequirementsFile(reqLegacyPath, reqIds, false);
    updated = updated.concat(legacyResult.updated);
    notFound = notFound.concat(legacyResult.notFound);
  }

  // If no files exist at all
  if (!authExists && !derivedExists && !legacyExists) {
    output({ updated: false, reason: 'No requirements file found (checked REQUIREMENTS.authoritative.md, REQUIREMENTS.derived.md, REQUIREMENTS.md)', ids: reqIds }, raw, 'no requirements file');
    return;
  }

  output({
    updated: updated.length > 0,
    marked_complete: updated,
    not_found: notFound,
    total: reqIds.length,
  }, raw, `${updated.length}/${reqIds.length} requirements marked complete`);
}

function cmdMilestoneComplete(cwd, version, options, raw) {
  if (!version) {
    error('version required for milestone complete (e.g., v1.0)');
  }

  const roadmapPath = path.join(cwd, '.planning', 'ROADMAP.md');
  // Support both legacy and new requirement files
  const reqAuthPath = path.join(cwd, '.planning', 'REQUIREMENTS.authoritative.md');
  const reqDerivedPath = path.join(cwd, '.planning', 'REQUIREMENTS.derived.md');
  const reqLegacyPath = path.join(cwd, '.planning', 'REQUIREMENTS.md');
  const statePath = path.join(cwd, '.planning', 'STATE.md');
  const milestonesPath = path.join(cwd, '.planning', 'MILESTONES.md');
  const archiveDir = path.join(cwd, '.planning', 'milestones');
  const phasesDir = path.join(cwd, '.planning', 'phases');
  const today = new Date().toISOString().split('T')[0];
  const milestoneName = options.name || version;

  // Ensure archive directory exists
  fs.mkdirSync(archiveDir, { recursive: true });

  // Gather stats from phases
  let phaseCount = 0;
  let totalPlans = 0;
  let totalTasks = 0;
  const accomplishments = [];

  try {
    const entries = fs.readdirSync(phasesDir, { withFileTypes: true });
    const dirs = entries.filter(e => e.isDirectory()).map(e => e.name).sort();

    for (const dir of dirs) {
      phaseCount++;
      const phaseFiles = fs.readdirSync(path.join(phasesDir, dir));
      const plans = phaseFiles.filter(f => f.endsWith('-PLAN.md') || f === 'PLAN.md');
      const summaries = phaseFiles.filter(f => f.endsWith('-SUMMARY.md') || f === 'SUMMARY.md');
      totalPlans += plans.length;

      // Extract one-liners from summaries
      for (const s of summaries) {
        try {
          const content = fs.readFileSync(path.join(phasesDir, dir, s), 'utf-8');
          const fm = extractFrontmatter(content);
          if (fm['one-liner']) {
            accomplishments.push(fm['one-liner']);
          }
          // Count tasks
          const taskMatches = content.match(/##\s*Task\s*\d+/gi) || [];
          totalTasks += taskMatches.length;
        } catch {}
      }
    }
  } catch {}

  // Archive ROADMAP.md
  if (fs.existsSync(roadmapPath)) {
    const roadmapContent = fs.readFileSync(roadmapPath, 'utf-8');
    fs.writeFileSync(path.join(archiveDir, `${version}-ROADMAP.md`), roadmapContent, 'utf-8');
  }

  // Archive REQUIREMENTS.authoritative.md
  if (fs.existsSync(reqAuthPath)) {
    const reqContent = fs.readFileSync(reqAuthPath, 'utf-8');
    const archiveHeader = `# Authoritative Requirements Archive: ${version} ${milestoneName}\n\n**Archived:** ${today}\n**Status:** SHIPPED\n\nFor current requirements, see \`.planning/REQUIREMENTS.authoritative.md\`.\n\n---\n\n`;
    fs.writeFileSync(path.join(archiveDir, `${version}-REQUIREMENTS.authoritative.md`), archiveHeader + reqContent, 'utf-8');
  }

  // Archive REQUIREMENTS.derived.md
  if (fs.existsSync(reqDerivedPath)) {
    const reqContent = fs.readFileSync(reqDerivedPath, 'utf-8');
    const archiveHeader = `# Derived Requirements Archive: ${version} ${milestoneName}\n\n**Archived:** ${today}\n**Status:** SHIPPED\n\nFor current requirements, see \`.planning/REQUIREMENTS.derived.md\`.\n\n---\n\n`;
    fs.writeFileSync(path.join(archiveDir, `${version}-REQUIREMENTS.derived.md`), archiveHeader + reqContent, 'utf-8');
  }

  // Legacy: Archive REQUIREMENTS.md if it exists and new files don't
  if (!fs.existsSync(reqAuthPath) && !fs.existsSync(reqDerivedPath) && fs.existsSync(reqLegacyPath)) {
    const reqContent = fs.readFileSync(reqLegacyPath, 'utf-8');
    const archiveHeader = `# Requirements Archive: ${version} ${milestoneName}\n\n**Archived:** ${today}\n**Status:** SHIPPED\n\nFor current requirements, see \`.planning/REQUIREMENTS.md\`.\n\n---\n\n`;
    fs.writeFileSync(path.join(archiveDir, `${version}-REQUIREMENTS.md`), archiveHeader + reqContent, 'utf-8');
  }

  // Archive audit file if exists
  const auditFile = path.join(cwd, '.planning', `${version}-MILESTONE-AUDIT.md`);
  if (fs.existsSync(auditFile)) {
    fs.renameSync(auditFile, path.join(archiveDir, `${version}-MILESTONE-AUDIT.md`));
  }

  // Create/append MILESTONES.md entry
  const accomplishmentsList = accomplishments.map(a => `- ${a}`).join('\n');
  const milestoneEntry = `## ${version} ${milestoneName} (Shipped: ${today})\n\n**Phases completed:** ${phaseCount} phases, ${totalPlans} plans, ${totalTasks} tasks\n\n**Key accomplishments:**\n${accomplishmentsList || '- (none recorded)'}\n\n---\n\n`;

  if (fs.existsSync(milestonesPath)) {
    const existing = fs.readFileSync(milestonesPath, 'utf-8');
    fs.writeFileSync(milestonesPath, existing + '\n' + milestoneEntry, 'utf-8');
  } else {
    fs.writeFileSync(milestonesPath, `# Milestones\n\n${milestoneEntry}`, 'utf-8');
  }

  // Update STATE.md
  if (fs.existsSync(statePath)) {
    let stateContent = fs.readFileSync(statePath, 'utf-8');
    stateContent = stateContent.replace(
      /(\*\*Status:\*\*\s*).*/,
      `$1${version} milestone complete`
    );
    stateContent = stateContent.replace(
      /(\*\*Last Activity:\*\*\s*).*/,
      `$1${today}`
    );
    stateContent = stateContent.replace(
      /(\*\*Last Activity Description:\*\*\s*).*/,
      `$1${version} milestone completed and archived`
    );
    fs.writeFileSync(statePath, stateContent, 'utf-8');
  }

  // Archive phase directories if requested
  let phasesArchived = false;
  if (options.archivePhases) {
    try {
      const phaseArchiveDir = path.join(archiveDir, `${version}-phases`);
      fs.mkdirSync(phaseArchiveDir, { recursive: true });

      const phaseEntries = fs.readdirSync(phasesDir, { withFileTypes: true });
      const phaseDirNames = phaseEntries.filter(e => e.isDirectory()).map(e => e.name);
      for (const dir of phaseDirNames) {
        fs.renameSync(path.join(phasesDir, dir), path.join(phaseArchiveDir, dir));
      }
      phasesArchived = phaseDirNames.length > 0;
    } catch {}
  }

  const result = {
    version,
    name: milestoneName,
    date: today,
    phases: phaseCount,
    plans: totalPlans,
    tasks: totalTasks,
    accomplishments,
    archived: {
      roadmap: fs.existsSync(path.join(archiveDir, `${version}-ROADMAP.md`)),
      requirements_authoritative: fs.existsSync(path.join(archiveDir, `${version}-REQUIREMENTS.authoritative.md`)),
      requirements_derived: fs.existsSync(path.join(archiveDir, `${version}-REQUIREMENTS.derived.md`)),
      requirements_legacy: fs.existsSync(path.join(archiveDir, `${version}-REQUIREMENTS.md`)),
      audit: fs.existsSync(path.join(archiveDir, `${version}-MILESTONE-AUDIT.md`)),
      phases: phasesArchived,
    },
    milestones_updated: true,
    state_updated: fs.existsSync(statePath),
  };

  output(result, raw);
}

module.exports = {
  cmdRequirementsMarkComplete,
  cmdMilestoneComplete,
};
