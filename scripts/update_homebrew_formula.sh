#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <version> <macos-arm64-sha256> <macos-x86_64-sha256> <linux-x86_64-sha256>" >&2
  exit 1
}

if [ "$#" -ne 4 ]; then
  usage
fi

version=$1
macos_arm64_sha256=$2
macos_x86_64_sha256=$3
linux_x86_64_sha256=$4

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
formula_path="$script_dir/../Formula/nixo.rb"

if [ ! -f "$formula_path" ]; then
  echo "error: formula not found at $formula_path" >&2
  exit 1
fi

ruby - "$formula_path" "$version" "$macos_arm64_sha256" "$macos_x86_64_sha256" "$linux_x86_64_sha256" <<'RUBY'
path, version, macos_arm64_sha256, macos_x86_64_sha256, linux_x86_64_sha256 = ARGV

lines = File.readlines(path, chomp: true)
sha_pattern = /^(\s*)sha256\s+"([0-9a-f]{64})"(\s*(?:#.*)?)$/
version_pattern = /^(\s*version ")([^"]+)(")(\s*(?:#.*)?)$/

def expect_exactly_one!(lines, label, pattern)
  matches = lines.each_index.select { |index| lines[index].match?(pattern) }
  raise "#{label}: expected exactly 1 match, found #{matches.length}" unless matches.length == 1
  matches.first
end

def expect_sha256!(value, label)
  return if value.match?(/\A[0-9a-f]{64}\z/)

  raise "#{label}: expected 64 lowercase hex characters"
end

def expect_version!(value)
  return if value.match?(/\A[0-9A-Za-z][0-9A-Za-z._-]*\z/)

  raise "version: expected only letters, numbers, dot, underscore, or hyphen"
end

def push_block(stack, line)
  case line
  when /^\s*on_macos do\b/
    stack << :macos
  when /^\s*on_linux do\b/
    stack << :linux
  when /^\s*on_arm do\b/
    stack << :arm
  when /^\s*on_intel do\b/
    stack << :intel
  when /^\s*(class|module|def|if|unless|case|while|until|for|begin)\b/,
       /^\s*test do\b/,
       /\bdo(?:\s*\|[^|]*\|)?\s*(?:#.*)?$/
    stack << :other
  end
end

def pop_block(stack, line)
  stack.pop if line.match?(/^\s*end\b/)
end

def block_key(stack)
  return :macos_arm64 if stack.include?(:macos) && stack.include?(:arm)
  return :macos_x86_64 if stack.include?(:macos) && stack.include?(:intel)
  return :linux_x86_64 if stack.include?(:linux) && stack.include?(:intel)

  nil
end

version_index = expect_exactly_one!(lines, "version line", version_pattern)
expect_version!(version)
expect_sha256!(macos_arm64_sha256, "macOS arm64 sha256")
expect_sha256!(macos_x86_64_sha256, "macOS x86_64 sha256")
expect_sha256!(linux_x86_64_sha256, "Linux x86_64 sha256")

target_shas = {
  macos_arm64: macos_arm64_sha256,
  macos_x86_64: macos_x86_64_sha256,
  linux_x86_64: linux_x86_64_sha256,
}
sha_indices = Hash.new { |hash, key| hash[key] = [] }
stack = []

lines.each_with_index do |line, index|
  push_block(stack, line)
  key = block_key(stack)
  sha_indices[key] << index if key && line.match?(sha_pattern)
  pop_block(stack, line)
end

target_shas.each do |key, new_sha256|
  matches = sha_indices[key]
  raise "#{key}: expected exactly 1 sha256 line, found #{matches.length}" unless matches.length == 1

  sha_index = matches.first
  indent, _old_sha, suffix = lines[sha_index].match(sha_pattern).captures
  lines[sha_index] = %(#{indent}sha256 "#{new_sha256}"#{suffix})
end

version_prefix, _old_version, version_quote, version_suffix = lines[version_index].match(version_pattern).captures
lines[version_index] = %(#{version_prefix}#{version}#{version_quote}#{version_suffix})

File.write(path, lines.join("\n") + "\n")
RUBY
