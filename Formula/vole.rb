# Homebrew formula for vole.
#
# Tap from this repo:
#   brew tap wukongnotnull/vole https://github.com/wukongnotnull/vole
#   brew install vole
#
# Local dev tap:
#   brew tap wukongnotnull/vole "$(pwd)"
#   brew install wukongnotnull/vole/vole
#
# Refresh stable url/sha256 after each release:
#   bash scripts/update-homebrew-formula.sh 0.0.10
class Vole < Formula
  desc "macOS cleanup and monitoring CLI (Mole-derived, GPL-3.0)"
  homepage "https://github.com/wukongnotnull/vole"
  license "GPL-3.0-only"
  version "1.1.0"
  depends_on :macos

  on_macos do
    on_arm do
      url "https://github.com/wukongnotnull/vole/releases/download/v1.1.0/vole-1.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "ffce2af6aa634b60a7221b1cfc78db6be04eff36a873eb91920f760cfc84bbad"
    end
    on_intel do
      url "https://github.com/wukongnotnull/vole/releases/download/v1.1.0/vole-1.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "5382c7c38f37a871c8e5b238f7549118d07c9605687814be82d0160f107d2b60"
    end
  end

  head do
    url "https://github.com/wukongnotnull/vole.git", branch: "main"
    depends_on "rust" => :build
  end

  def install
    if build.stable?
      prefix_dir = Dir["vole-#{version}-*"].first
      odie "unexpected tarball layout (expected vole-#{version}-<arch>)" if prefix_dir.nil?
      bin.install "#{prefix_dir}/bin/vole"
      (share/"vole/rules").install Dir["#{prefix_dir}/share/vole/rules/*.toml"]
    else
      system "cargo", "install", *std_cargo_install_args(path: "crates/vole-cli")
      (share/"vole").install "data/rules"
    end
  end

  def caveats
    <<~EOS
      Add to your shell rc:
        export VOLE_RULES_DIR="#{share}/vole/rules"

      Stable bottles are Developer ID signed and notarized (CLI has no staple;
      Gatekeeper may need a network check on first run). If Gatekeeper blocks:
        xattr -cr #{bin}/vole
    EOS
  end

  test do
    assert_match "vole", shell_output("#{bin}/vole --help")
    assert_predicate share/"vole/rules", :directory?
  end
end
