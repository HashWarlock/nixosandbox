class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixo"
  version "0.1.1"

  on_macos do
    on_arm do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-aarch64-apple-darwin.tar.gz"
      sha256 "52e0a8482a4528832a5b95a754f57f818524b4d5fa1c738f3372fc4b6f269879"
    end

    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-apple-darwin.tar.gz"
      sha256 "8e484b841373c619c27d0de72201c6acb81adc3a828ce54dd734ea1a7007c736"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "d4a51cb17981e947bdfd25bdf126db7e22c2d5bd5058db1caa6819d59555272a"
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
