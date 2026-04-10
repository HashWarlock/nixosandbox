class Nixo < Formula
  desc "Reproducible, isolated sandbox environments for AI coding agents"
  homepage "https://github.com/HashWarlock/nixo"
  version "0.1.1"
  depends_on "nix"

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
    libexec.install Dir["bin/*"]
    pkgshare.install Dir["flake"]

    flake_root = pkgshare/"flake"
    bin.write_env_script libexec/"nixo", "NIXOSANDBOX_FLAKE_ROOT" => flake_root
    bin.install_symlink "nixo" => "nixosandbox"
  end

  test do
    assert_predicate pkgshare/"flake/flake.nix", :exist?
    assert_predicate pkgshare/"flake/flake.lock", :exist?
    assert_match "nixo", shell_output("#{bin}/nixo --help")
    assert_match "nixo", shell_output("#{bin}/nixosandbox --help")
  end
end
