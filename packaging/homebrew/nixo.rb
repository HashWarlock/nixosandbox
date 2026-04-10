class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixosandbox"
  version "0.1.0"

  # Replace the placeholder sha256 values below with the published release checksums.
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

  on_linux do
    on_intel do
      url "https://github.com/HashWarlock/nixosandbox/releases/download/v#{version}/nixo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_LINUX_X86_64_SHA256"
    end
  end

  def install
    libexec.install Dir["bin/*"]
    pkgshare.install Dir["flake"]

    flake_root = pkgshare/"flake"
    bin.write_env_script libexec/"nixo", "NIXOSANDBOX_FLAKE_ROOT" => flake_root
    bin.install_symlink "nixo" => "nixosandbox"
  end

  test do
    assert_match "nixo", shell_output("#{bin}/nixo --help")
  end
end
