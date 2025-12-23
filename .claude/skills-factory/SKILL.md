---
name: skills-factory
description: >
  Creates new Claude Skills by learning from users through Socratic dialogue.
  Activates when user says "teach you", "learn this", "create a skill",
  "remember how to", describes a repeatable task to automate, or uploads
  a form/template asking to create a filling workflow.
---

# Skill Factory

Creates new Claude Skills by learning from users through structured dialogue,
following Anthropic's best practices for skill authoring.

## When to Activate

- User explicitly asks to teach: "teach you", "learn how to", "create a skill"
- User expresses repetition: "I always have to", "remember how to"
- User describes a task to automate or standardize
- User uploads a form/template wanting a filling workflow

## Do NOT Activate When

- User asks a normal question
- User wants help with a one-off task
- User is testing an existing skill

## Elicitation Process

Guide the user through these questions conversationally. Skip if already answered.

### Question 1: Goal
"What task do you want me to help with? Give me the high-level goal."

### Question 2: Trigger
"When should I use this skill? What words or situations should activate it?"

### Question 3: Concrete Example
"Walk me through a real example. What would you give me as input, and what should I produce?"

### Question 4: Complexity Assessment
Determine if this is a simple or complex skill:
- **Simple**: Text instructions only, no scripts needed
- **Complex**: Needs validation, scripts, templates, or multi-step workflow

For complex skills, ask:
"What resources should I include? (scripts, templates, reference docs)"

### Question 5: Edge Cases
"What should I do if something's missing or goes wrong?"

### Question 6: Confirmation
Summarize understanding:
- **Name**: {proposed-skill-name}
- **Triggers**: {when to activate}
- **Input**: {what user provides}
- **Output**: {what to produce}
- **Complexity**: Simple / Complex
- **Resources**: {if complex: scripts, references, assets}
- **Edge cases**: {how to handle problems}

Ask: "Does this capture what you want? Any adjustments?"

## Design Principles

Before creating a skill, consider these principles.

### Progressive Disclosure (3-Level Loading)

Skills use a three-level loading system to manage context efficiently:

1. **Metadata (name + description)** - Always in context (~100 words)
2. **SKILL.md body** - When skill triggers (<500 lines)
3. **Bundled resources** - As needed (references/, scripts/, assets/)

Keep SKILL.md lean. Move detailed content to references/ files.

### Degrees of Freedom

Match the level of specificity to the task's fragility:

- **High freedom**: Text instructions when multiple approaches are valid
- **Medium freedom**: Pseudocode when a preferred pattern exists
- **Low freedom**: Specific scripts when operations are fragile

### What NOT to Include

Skills should only contain essential files. Do NOT create:
- README.md (SKILL.md is the readme)
- INSTALLATION_GUIDE.md, QUICK_REFERENCE.md, CHANGELOG.md
- User-facing documentation (skills are for Claude, not humans)

## Helper Scripts

Use these scripts to streamline skill creation.

### Initialize a New Skill

```bash
python scripts/init_skill.py {skill-name}
python scripts/init_skill.py {skill-name} --complex  # with scripts
```

Creates the skill directory at `.claude/skills/learned/{skill-name}/`:
- SKILL.md with frontmatter template
- scripts/, references/, assets/ directories

### Validate a Skill

```bash
python scripts/validate_skill.py .claude/skills/learned/{skill-name}
```

Checks:
- YAML frontmatter format and required fields
- Name conventions (hyphen-case, max 64 chars)
- Description is third-person with trigger phrases
- SKILL.md under 500 lines

### Package a Skill

```bash
python scripts/package_skill.py .claude/skills/learned/{skill-name}
```

Creates a distributable `.skill` file (validates first, then packages).

## Reference Files

Consult these references based on skill needs:

- **`references/best-practices-checklist.md`** - Run before finalizing any skill
- **`references/workflows.md`** - Sequential and conditional workflow patterns
- **`references/output-patterns.md`** - Template and example patterns for output

## Creating the Skill

**Read `references/best-practices-checklist.md` before creating any skill.**

### Critical Requirements

1. **Description MUST be third person**:
   - "Fills forms...", "Analyzes data...", "Generates reports..."
   - NOT "Fill forms...", "I can help you...", "Use when..."

2. **Description MUST include trigger phrases**:
   - "Activates when user mentions X, Y, or Z"
   - NOT "Use when user needs help with forms"

3. **Complex skills MUST have**:
   - Workflow checklist (copyable)
   - Validation script (feedback loop)
   - Explicit dependencies section

### File Structure

**Simple Skills:**
```
.claude/skills/learned/{skill-name}/
└── SKILL.md
```

**Complex Skills:**
```
.claude/skills/learned/{skill-name}/
├── SKILL.md
├── scripts/
│   ├── validate_{name}.py    # Always validate first
│   └── {operation}.py
├── references/
│   └── {documentation}.md
└── assets/
    └── {templates}
```

### SKILL.md Template (Simple)

```markdown
---
name: {skill-name}
description: >
  {What it does - THIRD PERSON}. Activates when user {trigger phrases}.
---

# {Skill Title}

{One sentence explaining the purpose.}

## Workflow

1. {First step}
2. {Second step}
3. {Additional steps}

## Example

**Input**: {Example input}

**Output**: {Example output}

## Edge Cases

- {Situation}: {How to handle}
```

### SKILL.md Template (Complex)

```markdown
---
name: {skill-name}
description: >
  {What it does - THIRD PERSON}. Activates when user {trigger phrases}.
---

# {Skill Title}

{One sentence explaining the purpose.}

## Dependencies

Install before use:

\`\`\`bash
pip install {packages} --break-system-packages
\`\`\`

## Workflow

Copy this checklist and track progress:

\`\`\`
{Skill Name} Progress:
- [ ] Step 1: Collect information
- [ ] Step 2: Create data file
- [ ] Step 3: Validate data (run validate_{name}.py)
- [ ] Step 4: Fix any validation errors
- [ ] Step 5: Execute main operation
- [ ] Step 6: Generate output
- [ ] Step 7: Deliver to user
\`\`\`

## Quick Reference

\`\`\`bash
# Validate first
python scripts/validate_{name}.py data.json

# Then execute
python scripts/{operation}.py data.json output
\`\`\`

## Information Collection

{Sections for collecting required information}

## Data Structure

{JSON or other format example}

## Validation

**Always validate before proceeding.** Run:

\`\`\`bash
python scripts/validate_{name}.py data.json
\`\`\`

If validation fails, fix errors and re-run until it passes.

## Field Reference

See `references/{mapping}.md` for complete details.
```

### Validation Script Template

For complex skills, create `scripts/validate_{name}.py`:

```python
#!/usr/bin/env python3
"""Validate {skill} data before processing."""

import argparse
import json
import sys
from pathlib import Path


def validate(data: dict) -> tuple[list, list]:
    """Validate data. Returns (errors, warnings)."""
    errors = []
    warnings = []

    # Check required fields
    required = ["field1", "field2"]
    for field in required:
        if field not in data or not data[field]:
            errors.append(f"Missing required field: {field}")

    # Add domain-specific validations...

    return errors, warnings


def main():
    parser = argparse.ArgumentParser(description="Validate data")
    parser.add_argument("input_json", help="Path to JSON data file")
    args = parser.parse_args()

    # Load with helpful errors
    path = Path(args.input_json)
    if not path.exists():
        print(f"Error: File not found: {args.input_json}")
        print("Create a JSON file with the required data.")
        sys.exit(1)

    try:
        with open(path) as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON at line {e.lineno}: {e.msg}")
        sys.exit(1)

    errors, warnings = validate(data)

    # Print results
    for w in warnings:
        print(f"Warning: {w}")
    for e in errors:
        print(f"Error: {e}")

    if errors:
        print(f"\nValidation FAILED: {len(errors)} error(s)")
        sys.exit(1)
    else:
        print("Validation PASSED")
        sys.exit(0)


if __name__ == "__main__":
    main()
```

## After Creation

1. Verify files were created:
   ```bash
   ls -la .claude/skills/learned/{skill-name}/
   cat .claude/skills/learned/{skill-name}/SKILL.md
   ```

2. **Run the best practices checklist** from `references/best-practices-checklist.md`

3. Tell the user:
   "I've created the `{skill-name}` skill. Test it by saying:
   '{example prompt that would trigger it}'

   If it needs tweaking, just tell me what to change."

## Rules

- Always read `references/best-practices-checklist.md` before creating skills
- Description MUST be third person with trigger phrases
- Complex skills MUST have validation scripts and workflow checklists
- Keep SKILL.md under 500 lines (use references/ for details)
- Use kebab-case for names (e.g., `expense-reviewer`)
- Ask for confirmation before writing files
- Test scripts by running them after creation

## Updating Existing Skills

If user wants to modify a skill:
1. Read current skill: `cat .claude/skills/learned/{name}/SKILL.md`
2. Ask what needs to change
3. Verify changes follow best practices checklist
4. Rewrite files with updates
5. Run validation if applicable
6. Confirm the change
