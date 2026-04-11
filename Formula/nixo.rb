class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixo"
  version "0.1.4"

  on_macos do
    on_arm do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-aarch64-apple-darwin.tar.gz"
      sha256 "70e43515e50de21526a4d8996e0c46b09f1f5cf2444aaf94c73f1f59358e6550"
    end

    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-apple-darwin.tar.gz"
      sha256 "6ba350e03ce42ee97e2780be10c687415ee02c93175c41b8af81f1b0d4ff91a5"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/HashWarlock/nixo/releases/download/v#{version}/nixo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "4daf1061989ee625faa5c815a1f231bc9dbe3baf0f65faf8a88216e9a361b71e"
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
