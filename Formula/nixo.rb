class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixo"
  version "0.1.3"

  on_macos do
    on_arm do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-aarch64-apple-darwin.tar.gz"
      sha256 "618a37e344b77fbf1d7246d7789f1c2d51645bda69825da610eb1728d2748a59"
    end

    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-apple-darwin.tar.gz"
      sha256 "01967dc3cceca365c026b324bb412be4b6faa2a5b59b82e482a5ff0e7816bc4e"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "1f5a02a5402dcd2e9865c894d756cc3e2d3530d9b33cae231836841075a33e75"
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
