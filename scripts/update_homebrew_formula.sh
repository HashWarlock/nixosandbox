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

def expect_exactly_one!(lines, label, pattern)
  matches = lines.each_index.select { |index| lines[index].match?(pattern) }
  raise "#{label}: expected exactly 1 match, found #{matches.length}" unless matches.length == 1
  matches.first
end

version_index = expect_exactly_one!(lines, "version line", /^\s*version ".*"$/)

targets = [
  [
    "macOS arm64",
    %r{\A\s{6}url "https://github\.com/HashWarlock/nixo/releases/download/v#\{version\}/nixo-aarch64-apple-darwin\.tar\.gz"\z},
    macos_arm64_sha256,
  ],
  [
    "macOS x86_64",
    %r{\A\s{6}url "https://github\.com/HashWarlock/nixo/releases/download/v#\{version\}/nixo-x86_64-apple-darwin\.tar\.gz"\z},
    macos_x86_64_sha256,
  ],
  [
    "Linux x86_64",
    %r{\A\s{6}url "https://github\.com/HashWarlock/nixo/releases/download/v#\{version\}/nixo-x86_64-unknown-linux-gnu\.tar\.gz"\z},
    linux_x86_64_sha256,
  ],
]

targets.each do |label, url_pattern, new_sha256|
  url_index = expect_exactly_one!(lines, "#{label} url line", url_pattern)
  sha_index = url_index + 1

  unless sha_index < lines.length && lines[sha_index].match?(/^\s{6}sha256 "[0-9a-f]{64}"$/)
    raise "#{label}: expected a sha256 line immediately after the url line"
  end

  lines[sha_index] = %(      sha256 "#{new_sha256}")
end

lines[version_index] = %(  version "#{version}")

File.write(path, lines.join("\n") + "\n")
RUBY
