class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixo"
  version "0.1.2"

  on_macos do
    on_arm do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-aarch64-apple-darwin.tar.gz"
      sha256 "3404af89d08e43d860554b8e0263d9a410540ec2d991bd4176f6a6d4667707c0"
    end

    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-apple-darwin.tar.gz"
      sha256 "af6348684a4fb5bdfe9812699f8fe8024dc9c15dbc8b463f30fb1404bf5592ec"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "9518a8efa7a79c8345d053ca36d6f35a435ccf862f6b870455969cc7e169bb53"
    end
  end

  def install
    bin.install "bin/nixo", "bin/nixosandbox"
    prefix.install "flake/flake.nix", "flake/flake.lock"
    prefix.install "flake/nix"
  end

  test do
    assert_predicate prefix/"flake.nix", :exist?
    assert_predicate prefix/"flake.lock", :exist?
    assert_predicate prefix/"nix", :exist?
    assert_match "nixo", shell_output("#{bin}/nixo --help")
    assert_match "nixo", shell_output("#{bin}/nixosandbox --help")
  end
end
