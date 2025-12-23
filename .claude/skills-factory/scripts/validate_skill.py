#!/usr/bin/env python3
"""
Skill Validator - Enhanced validation for skills-factory

Validates skill structure, frontmatter, naming conventions, and content quality.

Usage:
    validate_skill.py <skill_directory>

Example:
    validate_skill.py .claude/skills/learned/my-skill
"""

import sys
import re
import yaml
from pathlib import Path


# Third-person verb patterns that descriptions should start with
THIRD_PERSON_VERBS = [
    'fills', 'creates', 'generates', 'analyzes', 'processes', 'extracts',
    'converts', 'validates', 'handles', 'manages', 'provides', 'enables',
    'supports', 'automates', 'transforms', 'parses', 'builds', 'formats',
    'guides', 'assists', 'helps', 'fetches', 'retrieves', 'sends', 'posts',
    'queries', 'searches', 'finds', 'locates', 'identifies', 'detects',
    'monitors', 'tracks', 'logs', 'records', 'stores', 'saves', 'loads',
    'reads', 'writes', 'edits', 'modifies', 'updates', 'deletes', 'removes',
    'adds', 'inserts', 'appends', 'merges', 'splits', 'combines', 'joins',
    'filters', 'sorts', 'groups', 'aggregates', 'calculates', 'computes',
    'measures', 'counts', 'summarizes', 'reports', 'displays', 'shows',
    'renders', 'visualizes', 'exports', 'imports', 'syncs', 'synchronizes',
    'connects', 'links', 'integrates', 'orchestrates', 'coordinates',
]

# Trigger phrase patterns to look for in descriptions
TRIGGER_PATTERNS = [
    r'activates when',
    r'triggers when',
    r'use when',
    r'invoke when',
    r'applies when',
    r'for when',
]

# Allowed frontmatter properties
ALLOWED_PROPERTIES = {'name', 'description', 'license', 'allowed-tools', 'metadata'}

# Limits
MAX_NAME_LENGTH = 64
MAX_DESCRIPTION_LENGTH = 1024
MAX_SKILL_LINES = 500


def validate_skill(skill_path: str) -> tuple[bool, list[str], list[str]]:
    """
    Validate a skill directory.

    Returns:
        (is_valid, errors, warnings)
    """
    skill_path = Path(skill_path)
    errors = []
    warnings = []

    # Check SKILL.md exists
    skill_md = skill_path / 'SKILL.md'
    if not skill_md.exists():
        return False, ["SKILL.md not found"], warnings

    # Read content
    content = skill_md.read_text()
    lines = content.splitlines()

    # Check line count
    if len(lines) > MAX_SKILL_LINES:
        warnings.append(
            f"SKILL.md is {len(lines)} lines (recommended max: {MAX_SKILL_LINES}). "
            "Consider moving detailed content to references/ files."
        )

    # Check frontmatter exists
    if not content.startswith('---'):
        return False, ["No YAML frontmatter found (must start with ---)"], warnings

    # Extract frontmatter
    match = re.match(r'^---\n(.*?)\n---', content, re.DOTALL)
    if not match:
        return False, ["Invalid frontmatter format (missing closing ---)"], warnings

    frontmatter_text = match.group(1)

    # Parse YAML frontmatter
    try:
        frontmatter = yaml.safe_load(frontmatter_text)
        if not isinstance(frontmatter, dict):
            return False, ["Frontmatter must be a YAML dictionary"], warnings
    except yaml.YAMLError as e:
        return False, [f"Invalid YAML in frontmatter: {e}"], warnings

    # Check for unexpected properties
    unexpected_keys = set(frontmatter.keys()) - ALLOWED_PROPERTIES
    if unexpected_keys:
        errors.append(
            f"Unexpected key(s) in frontmatter: {', '.join(sorted(unexpected_keys))}. "
            f"Allowed: {', '.join(sorted(ALLOWED_PROPERTIES))}"
        )

    # Check required fields
    if 'name' not in frontmatter:
        errors.append("Missing 'name' in frontmatter")
    if 'description' not in frontmatter:
        errors.append("Missing 'description' in frontmatter")

    # Validate name
    name = frontmatter.get('name', '')
    if not isinstance(name, str):
        errors.append(f"Name must be a string, got {type(name).__name__}")
    else:
        name = name.strip()
        if name:
            # Check hyphen-case format
            if not re.match(r'^[a-z0-9-]+$', name):
                errors.append(
                    f"Name '{name}' must be hyphen-case "
                    "(lowercase letters, digits, and hyphens only)"
                )
            if name.startswith('-') or name.endswith('-') or '--' in name:
                errors.append(
                    f"Name '{name}' cannot start/end with hyphen "
                    "or contain consecutive hyphens"
                )
            if len(name) > MAX_NAME_LENGTH:
                errors.append(
                    f"Name is too long ({len(name)} chars). "
                    f"Maximum: {MAX_NAME_LENGTH}"
                )
            # Check directory name matches
            if skill_path.name != name:
                warnings.append(
                    f"Directory name '{skill_path.name}' doesn't match "
                    f"skill name '{name}'"
                )

    # Validate description
    description = frontmatter.get('description', '')
    if not isinstance(description, str):
        errors.append(f"Description must be a string, got {type(description).__name__}")
    else:
        description = description.strip()
        if description:
            # Check for angle brackets
            if '<' in description or '>' in description:
                errors.append("Description cannot contain angle brackets (< or >)")

            # Check length
            if len(description) > MAX_DESCRIPTION_LENGTH:
                errors.append(
                    f"Description is too long ({len(description)} chars). "
                    f"Maximum: {MAX_DESCRIPTION_LENGTH}"
                )

            # Check third-person voice
            first_word = description.split()[0].lower() if description.split() else ''
            if first_word not in THIRD_PERSON_VERBS:
                warnings.append(
                    f"Description should start with a third-person verb "
                    f"(e.g., 'Fills', 'Creates', 'Analyzes'). "
                    f"Found: '{first_word}'"
                )

            # Check for trigger phrases
            has_trigger = any(
                re.search(pattern, description, re.IGNORECASE)
                for pattern in TRIGGER_PATTERNS
            )
            if not has_trigger:
                warnings.append(
                    "Description should include trigger phrases "
                    "(e.g., 'Activates when user mentions...')"
                )

    # Check for validation script in complex skills
    scripts_dir = skill_path / 'scripts'
    if scripts_dir.exists() and scripts_dir.is_dir():
        script_files = list(scripts_dir.glob('*.py'))
        has_validate_script = any('validate' in f.name.lower() for f in script_files)
        if script_files and not has_validate_script:
            warnings.append(
                "Complex skills with scripts should include a validation script "
                "(e.g., scripts/validate_data.py)"
            )

    is_valid = len(errors) == 0
    return is_valid, errors, warnings


def main():
    if len(sys.argv) != 2:
        print("Usage: validate_skill.py <skill_directory>")
        print("\nExample:")
        print("  validate_skill.py .claude/skills/learned/my-skill")
        sys.exit(1)

    skill_path = sys.argv[1]

    if not Path(skill_path).exists():
        print(f"Error: Path not found: {skill_path}")
        sys.exit(1)

    if not Path(skill_path).is_dir():
        print(f"Error: Not a directory: {skill_path}")
        sys.exit(1)

    is_valid, errors, warnings = validate_skill(skill_path)

    # Print warnings
    for warning in warnings:
        print(f"Warning: {warning}")

    # Print errors
    for error in errors:
        print(f"Error: {error}")

    # Print summary
    if is_valid:
        if warnings:
            print(f"\nValidation PASSED with {len(warnings)} warning(s)")
        else:
            print("\nValidation PASSED")
        sys.exit(0)
    else:
        print(f"\nValidation FAILED: {len(errors)} error(s)")
        sys.exit(1)


if __name__ == "__main__":
    main()
