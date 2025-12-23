# Skill Authoring Best Practices Checklist

Use this checklist when creating skills to ensure compliance with Anthropic's guidelines.

## CRITICAL Requirements

### Description (YAML Frontmatter)

- [ ] **Third person voice**: "Fills forms..." NOT "Fill forms..." or "I can help you..."
- [ ] **Includes trigger phrases**: "Activates when user mentions X, Y, Z"
- [ ] **Specific, not vague**: Include key terms users would say
- [ ] **Under 1024 characters**

**Good Example:**
```yaml
description: >
  Fills California DE-111 Petition for Probate forms through guided data collection
  and automated PDF form filling. Activates when user mentions California probate
  petition, DE-111 form, petition for letters of administration, or petition for
  probate of will.
```

**Bad Examples:**
```yaml
# Wrong - imperative voice
description: Fill out probate forms. Use when user needs help with forms.

# Wrong - first person
description: I can help you fill out California probate forms.

# Wrong - too vague
description: Helps with legal documents.
```

### Name (YAML Frontmatter)

- [ ] Lowercase letters, numbers, and hyphens only
- [ ] Maximum 64 characters
- [ ] No reserved words ("anthropic", "claude")
- [ ] Prefer gerund form: `filling-forms`, `analyzing-data`

## Structure Requirements

### SKILL.md Body

- [ ] Under 500 lines (use progressive disclosure for larger content)
- [ ] No time-sensitive information
- [ ] Consistent terminology throughout
- [ ] File references are one level deep (not nested)

### For Complex Workflows

- [ ] **Include copyable checklist** that Claude can track progress with:

```markdown
## Workflow

Copy this checklist and track progress:

\`\`\`
Task Progress:
- [ ] Step 1: Collect information
- [ ] Step 2: Validate data
- [ ] Step 3: Process data
- [ ] Step 4: Generate output
- [ ] Step 5: Deliver to user
\`\`\`
```

### For Skills with Scripts

- [ ] **Implement feedback loop** (validate -> fix -> repeat):
  - Add validation script that runs BEFORE main operation
  - Validation provides specific error messages
  - Workflow includes "fix errors and re-validate" step

- [ ] **"Solve, don't punt"** - scripts handle errors explicitly:
```python
# Good - handles error with helpful message
if not path.exists():
    print(f"Error: File not found: {path}")
    print("Create the file first. See references/schema.md for format.")
    sys.exit(1)

# Bad - punts to Claude
data = open(path).read()  # Just fails
```

- [ ] **List dependencies explicitly** in SKILL.md:
```markdown
## Dependencies

Install before use:
\`\`\`bash
pip install pypdf pdf2image --break-system-packages
\`\`\`
```

- [ ] **No "voodoo constants"** - all magic numbers justified:
```python
# Good
TIMEOUT = 30  # HTTP requests typically complete within 30 seconds

# Bad
TIMEOUT = 47  # Why 47?
```

## Directory Structure

### Simple Skills (instructions only)
```
skill-name/
└── SKILL.md
```

### Complex Skills (with resources)
```
skill-name/
├── SKILL.md                 # Main instructions (<500 lines)
├── scripts/                 # Utility scripts
│   ├── validate_data.py     # Validation (feedback loop)
│   ├── process_data.py      # Main operation
│   └── generate_output.py   # Output generation
├── references/              # Documentation (loaded as needed)
│   └── field_mapping.md
└── assets/                  # Templates, forms (not loaded into context)
    └── template.pdf
```

## Progressive Disclosure

- [ ] **SKILL.md under 500 lines** - move detailed content to references/
- [ ] **Complex details in references/** - keep SKILL.md lean
- [ ] **One-level-deep references** - don't nest reference files
- [ ] **Large files have TOC** - for references >100 lines, add table of contents

### 3-Level Loading System

Skills load content progressively:
1. **Metadata** (~100 words): name + description - always in context
2. **SKILL.md body** (<500 lines): loaded when skill triggers
3. **Bundled resources**: loaded only as needed

## Degrees of Freedom

Match specificity to task fragility:

- [ ] **High freedom (text instructions)**: When multiple valid approaches exist
- [ ] **Medium freedom (pseudocode)**: When a preferred pattern exists with variation
- [ ] **Low freedom (specific scripts)**: When operations are fragile or error-prone

## What NOT to Include

Skills should only contain essential files. Do NOT create:

- [ ] No README.md (SKILL.md is the readme)
- [ ] No INSTALLATION_GUIDE.md
- [ ] No QUICK_REFERENCE.md
- [ ] No CHANGELOG.md
- [ ] No user-facing documentation (skills are for Claude, not humans)

## Resource Organization

### scripts/
- Executable code for repeated operations
- Use when: Same code is rewritten repeatedly
- Benefits: Token efficient, deterministic, can execute without loading context

### references/
- Documentation loaded into context as needed
- Use when: Information needed while working (not just metadata)
- Examples: API docs, schemas, detailed workflows

### assets/
- Files used in output, NOT loaded into context
- Use when: Templates or boilerplate needed in final output
- Examples: .pptx templates, logos, fonts, boilerplate code

## Anti-Patterns to Avoid

- Windows-style paths (`scripts\helper.py`)
- Offering too many options without a default
- Deeply nested file references
- Time-sensitive information ("after August 2025...")
- Inconsistent terminology
- Over-explaining things Claude already knows
- Creating auxiliary documentation (README, CHANGELOG, etc.)
