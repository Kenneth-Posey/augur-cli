# Changelog

## Summary
Formatting cleanup (em dash -> hyphen), version bumps (app/core/domain to 5.1.0), .gitignore additions, and documentation/code comment fixes.

## Issues Resolved
None - this is a bulk formatting and version-alignment pass.

## Root Causes
N/A

## Solutions
- Replaced all em dash (—) characters in markdown and code comments with regular hyphen (-)
- Bumped crate versions: augur-app 4.1.0→5.1.0, augur-core 4.0.0→5.1.0, augur-domain 4.1.0→5.1.0
- Updated .gitignore with comments about secrets and state files
- Added rule to copilot-instructions: never mention github copilot in commit messages or comments
- Minor docs and code comment refinements (INSTALL.md, write_section.rs, agent markdown, skills)

## Files Changed
36 files modified across .github/, crate sources, configs, docs, public-html/, and root configs.

## Status
Committed