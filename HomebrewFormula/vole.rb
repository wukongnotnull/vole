# Homebrew formula for vole (source / HEAD install until stable sha256 is pinned).
#
#   brew install --HEAD ./HomebrewFormula/vole.rb
#
# After GitHub Release v0.0.1, add stable block:
#   url "https://github.com/wukongnotnull/vole/archive/refs/tags/v0.0.1.tar.gz"
#   sha256 "<from: curl -L url | shasum -a 256>"
class Vole < Formula
  desc "macOS cleanup and monitoring CLI (Mole-derived, GPL-3.0)"
  homepage "https://github.com/wukongnotnull/vole"
  license "GPL-3.0-only"
  head "https://github.com/wukongnotnull/vole.git", branch: "main"

  depends_on "rust" => :build
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_install_args(path: "crates/vole-cli")
    (share/"vole").install "data/rules"
  end

  def caveats
    <<~EOS
      export VOLE_RULES_DIR="#{share}/vole/rules"
    EOS
  end

  test do
    assert_match "vole", shell_output("#{bin}/vole --help")
  end
end
