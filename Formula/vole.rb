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
  version "0.0.10"
  depends_on :macos

  on_macos do
    on_arm do
      url "https://github.com/wukongnotnull/vole/releases/download/v0.0.10/vole-0.0.10-aarch64-apple-darwin.tar.gz"
      sha256 "d8fbdd76f5d370a2d6953a74a2e1f825f80c6a242c5fb17eaf21e2a1f1309234"
    end
    on_intel do
      url "https://github.com/wukongnotnull/vole/releases/download/v0.0.10/vole-0.0.10-x86_64-apple-darwin.tar.gz"
      sha256 "f2b80acb25c6390a0a2fbb27a48828122bd4891bebb4a3ffef7ea5d50aaef4a0"
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
