#!/usr/bin/env python3
"""
Skill Packager - Creates a distributable .skill file

Usage:
    package_skill.py <path/to/skill-folder> [output-directory]

Example:
    package_skill.py .claude/skills/learned/my-skill
    package_skill.py .claude/skills/learned/my-skill ./dist
"""

import sys
import zipfile
from pathlib import Path

# Import validation from sibling module
script_dir = Path(__file__).parent
sys.path.insert(0, str(script_dir))
from validate_skill import validate_skill


def package_skill(skill_path: str, output_dir: str = None) -> Path | None:
    """
    Package a skill folder into a .skill file.

    Args:
        skill_path: Path to the skill folder
        output_dir: Optional output directory (defaults to current directory)

    Returns:
        Path to created .skill file, or None if error
    """
    skill_path = Path(skill_path).resolve()

    # Validate skill folder exists
    if not skill_path.exists():
        print(f"Error: Skill folder not found: {skill_path}")
        return None

    if not skill_path.is_dir():
        print(f"Error: Path is not a directory: {skill_path}")
        return None

    # Validate SKILL.md exists
    skill_md = skill_path / "SKILL.md"
    if not skill_md.exists():
        print(f"Error: SKILL.md not found in {skill_path}")
        return None

    # Run validation before packaging
    print("Validating skill...")
    is_valid, errors, warnings = validate_skill(skill_path)

    # Print warnings
    for warning in warnings:
        print(f"Warning: {warning}")

    # Print errors
    for error in errors:
        print(f"Error: {error}")

    if not is_valid:
        print("\nValidation failed. Please fix errors before packaging.")
        return None

    print("Validation passed\n")

    # Determine output location
    skill_name = skill_path.name
    if output_dir:
        output_path = Path(output_dir).resolve()
        output_path.mkdir(parents=True, exist_ok=True)
    else:
        output_path = Path.cwd()

    skill_filename = output_path / f"{skill_name}.skill"

    # Create the .skill file (zip format)
    try:
        file_count = 0
        with zipfile.ZipFile(skill_filename, 'w', zipfile.ZIP_DEFLATED) as zipf:
            # Walk through the skill directory
            for file_path in skill_path.rglob('*'):
                if file_path.is_file():
                    # Skip hidden files and common unwanted files
                    if file_path.name.startswith('.'):
                        continue
                    if file_path.name == '__pycache__':
                        continue
                    if file_path.suffix == '.pyc':
                        continue

                    # Calculate relative path within the zip
                    arcname = file_path.relative_to(skill_path.parent)
                    zipf.write(file_path, arcname)
                    print(f"  Added: {arcname}")
                    file_count += 1

        print(f"\nPackaged {file_count} files to: {skill_filename}")
        return skill_filename

    except Exception as e:
        print(f"Error creating .skill file: {e}")
        return None


def main():
    if len(sys.argv) < 2:
        print("Usage: package_skill.py <path/to/skill-folder> [output-directory]")
        print("\nExample:")
        print("  package_skill.py .claude/skills/learned/my-skill")
        print("  package_skill.py .claude/skills/learned/my-skill ./dist")
        sys.exit(1)

    skill_path = sys.argv[1]
    output_dir = sys.argv[2] if len(sys.argv) > 2 else None

    print(f"Packaging skill: {skill_path}")
    if output_dir:
        print(f"Output directory: {output_dir}")
    print()

    result = package_skill(skill_path, output_dir)
    sys.exit(0 if result else 1)


if __name__ == "__main__":
    main()
