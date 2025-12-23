#!/usr/bin/env python3
"""
Skill Initializer - Creates a new skill from template

Usage:
    init_skill.py <skill-name> [--path <path>]

Examples:
    init_skill.py my-new-skill
    init_skill.py data-analyzer --path /custom/location

By default, skills are created in .claude/skills/learned/
"""

import sys
import re
from pathlib import Path


# Default output path for learned skills
DEFAULT_PATH = ".claude/skills/learned"


SKILL_TEMPLATE = '''---
name: {skill_name}
description: >
  [What it does - THIRD PERSON]. Activates when user [trigger phrases].
---

# {skill_title}

[One sentence explaining the purpose.]

## Workflow

1. [First step]
2. [Second step]
3. [Additional steps]

## Example

**Input**: [Example input]

**Output**: [Example output]

## Edge Cases

- [Situation]: [How to handle]
'''


COMPLEX_SKILL_TEMPLATE = '''---
name: {skill_name}
description: >
  [What it does - THIRD PERSON]. Activates when user [trigger phrases].
---

# {skill_title}

[One sentence explaining the purpose.]

## Dependencies

Install before use:

```bash
pip install [packages] --break-system-packages
```

## Workflow

Copy this checklist and track progress:

```
{skill_title} Progress:
- [ ] Step 1: Collect information
- [ ] Step 2: Create data file
- [ ] Step 3: Validate data
- [ ] Step 4: Fix any validation errors
- [ ] Step 5: Execute main operation
- [ ] Step 6: Deliver to user
```

## Quick Reference

```bash
# Validate first
python scripts/validate_data.py data.json

# Then execute
python scripts/process.py data.json output
```

## Information Collection

[Sections for collecting required information]

## Data Structure

```json
{{
  "field1": "value1",
  "field2": "value2"
}}
```

## Validation

**Always validate before proceeding.** Run:

```bash
python scripts/validate_data.py data.json
```

If validation fails, fix errors and re-run until it passes.
'''


EXAMPLE_SCRIPT = '''#!/usr/bin/env python3
"""
Example helper script for {skill_name}

Replace with actual implementation or delete if not needed.
"""

import argparse
import json
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description="Process data for {skill_name}")
    parser.add_argument("input_file", help="Path to input JSON file")
    args = parser.parse_args()

    path = Path(args.input_file)
    if not path.exists():
        print(f"Error: File not found: {{args.input_file}}")
        sys.exit(1)

    try:
        with open(path) as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON at line {{e.lineno}}: {{e.msg}}")
        sys.exit(1)

    # TODO: Add processing logic here
    print(f"Loaded data with {{len(data)}} keys")


if __name__ == "__main__":
    main()
'''


VALIDATE_SCRIPT = '''#!/usr/bin/env python3
"""
Validate {skill_name} data before processing.
"""

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
            errors.append(f"Missing required field: {{field}}")

    # TODO: Add domain-specific validations

    return errors, warnings


def main():
    parser = argparse.ArgumentParser(description="Validate data")
    parser.add_argument("input_json", help="Path to JSON data file")
    args = parser.parse_args()

    path = Path(args.input_json)
    if not path.exists():
        print(f"Error: File not found: {{args.input_json}}")
        print("Create a JSON file with the required data.")
        sys.exit(1)

    try:
        with open(path) as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON at line {{e.lineno}}: {{e.msg}}")
        sys.exit(1)

    errors, warnings = validate(data)

    for w in warnings:
        print(f"Warning: {{w}}")
    for e in errors:
        print(f"Error: {{e}}")

    if errors:
        print(f"\\nValidation FAILED: {{len(errors)}} error(s)")
        sys.exit(1)
    else:
        print("Validation PASSED")
        sys.exit(0)


if __name__ == "__main__":
    main()
'''


EXAMPLE_REFERENCE = """# Reference Documentation for {skill_title}

This is a placeholder for detailed reference documentation.
Replace with actual reference content or delete if not needed.

## When to Use Reference Files

Reference files are ideal for:
- Comprehensive API documentation
- Detailed workflow guides
- Complex multi-step processes
- Information too lengthy for main SKILL.md
- Content only needed for specific use cases
"""


def title_case_skill_name(skill_name: str) -> str:
    """Convert hyphenated skill name to Title Case."""
    return ' '.join(word.capitalize() for word in skill_name.split('-'))


def validate_skill_name(skill_name: str) -> tuple[bool, str]:
    """Validate skill name format."""
    if not skill_name:
        return False, "Skill name cannot be empty"

    if not re.match(r'^[a-z0-9-]+$', skill_name):
        return False, "Skill name must be hyphen-case (lowercase letters, digits, hyphens only)"

    if skill_name.startswith('-') or skill_name.endswith('-') or '--' in skill_name:
        return False, "Skill name cannot start/end with hyphen or contain consecutive hyphens"

    if len(skill_name) > 64:
        return False, f"Skill name too long ({len(skill_name)} chars). Maximum: 64"

    return True, ""


def init_skill(skill_name: str, path: str, complex_mode: bool = False) -> Path | None:
    """
    Initialize a new skill directory with template files.

    Args:
        skill_name: Name of the skill (hyphen-case)
        path: Parent directory where skill folder will be created
        complex_mode: If True, use complex skill template with scripts

    Returns:
        Path to created skill directory, or None if error
    """
    # Validate skill name
    valid, error = validate_skill_name(skill_name)
    if not valid:
        print(f"Error: {error}")
        return None

    # Determine skill directory path
    skill_dir = Path(path).resolve() / skill_name

    # Check if directory already exists
    if skill_dir.exists():
        print(f"Error: Skill directory already exists: {skill_dir}")
        return None

    # Create skill directory
    try:
        skill_dir.mkdir(parents=True, exist_ok=False)
        print(f"Created skill directory: {skill_dir}")
    except Exception as e:
        print(f"Error creating directory: {e}")
        return None

    # Create SKILL.md from template
    skill_title = title_case_skill_name(skill_name)
    template = COMPLEX_SKILL_TEMPLATE if complex_mode else SKILL_TEMPLATE
    skill_content = template.format(
        skill_name=skill_name,
        skill_title=skill_title
    )

    skill_md_path = skill_dir / 'SKILL.md'
    try:
        skill_md_path.write_text(skill_content)
        print("Created SKILL.md")
    except Exception as e:
        print(f"Error creating SKILL.md: {e}")
        return None

    # Create resource directories with example files
    try:
        # Create scripts/ directory
        scripts_dir = skill_dir / 'scripts'
        scripts_dir.mkdir(exist_ok=True)

        if complex_mode:
            # Add example scripts for complex skills
            example_script = scripts_dir / 'process.py'
            example_script.write_text(EXAMPLE_SCRIPT.format(skill_name=skill_name))
            example_script.chmod(0o755)
            print("Created scripts/process.py")

            validate_script = scripts_dir / 'validate_data.py'
            validate_script.write_text(VALIDATE_SCRIPT.format(skill_name=skill_name))
            validate_script.chmod(0o755)
            print("Created scripts/validate_data.py")
        else:
            # Create empty .gitkeep
            (scripts_dir / '.gitkeep').touch()
            print("Created scripts/ directory")

        # Create references/ directory
        references_dir = skill_dir / 'references'
        references_dir.mkdir(exist_ok=True)
        example_reference = references_dir / 'reference.md'
        example_reference.write_text(EXAMPLE_REFERENCE.format(skill_title=skill_title))
        print("Created references/reference.md")

        # Create assets/ directory
        assets_dir = skill_dir / 'assets'
        assets_dir.mkdir(exist_ok=True)
        (assets_dir / '.gitkeep').touch()
        print("Created assets/ directory")

    except Exception as e:
        print(f"Error creating resource directories: {e}")
        return None

    # Print next steps
    print(f"\nSkill '{skill_name}' initialized at {skill_dir}")
    print("\nNext steps:")
    print("1. Edit SKILL.md to add your workflow and examples")
    print("2. Update the description with third-person voice and trigger phrases")
    print("3. Add scripts, references, or assets as needed")
    print("4. Run validate_skill.py to check the skill structure")

    return skill_dir


def main():
    # Parse arguments
    skill_name = None
    path = DEFAULT_PATH
    complex_mode = False

    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == '--path' and i + 1 < len(args):
            path = args[i + 1]
            i += 2
        elif args[i] == '--complex':
            complex_mode = True
            i += 1
        elif not args[i].startswith('-'):
            skill_name = args[i]
            i += 1
        else:
            print(f"Unknown option: {args[i]}")
            sys.exit(1)

    if not skill_name:
        print("Usage: init_skill.py <skill-name> [--path <path>] [--complex]")
        print("\nOptions:")
        print(f"  --path <path>  Output directory (default: {DEFAULT_PATH})")
        print("  --complex      Use complex skill template with scripts")
        print("\nExamples:")
        print("  init_skill.py my-new-skill")
        print("  init_skill.py data-analyzer --complex")
        print("  init_skill.py custom-skill --path /custom/location")
        sys.exit(1)

    print(f"Initializing skill: {skill_name}")
    print(f"Location: {path}")
    if complex_mode:
        print("Mode: Complex (with scripts)")
    print()

    result = init_skill(skill_name, path, complex_mode)
    sys.exit(0 if result else 1)


if __name__ == "__main__":
    main()
