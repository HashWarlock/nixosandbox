class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixosandbox"
  version "0.1.0"

  on_macos do
    on_arm do
      url "https://github.com/HashWarlock/nixosandbox/releases/download/v#{version}/nixo-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_SHA256"
    end

    on_intel do
      url "https://github.com/HashWarlock/nixosandbox/releases/download/v#{version}/nixo-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  def install
    bin.install "nixo"
    bin.install_symlink "nixo" => "nixosandbox"
  end

  test do
    assert_match "nixo", shell_output("#{bin}/nixo --help")
  end
end
