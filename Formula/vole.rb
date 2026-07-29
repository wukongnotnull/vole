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
#   bash scripts/update-homebrew-formula.sh 0.0.7
class Vole < Formula
  desc "macOS cleanup and monitoring CLI (Mole-derived, GPL-3.0)"
  homepage "https://github.com/wukongnotnull/vole"
  license "GPL-3.0-only"
  version "0.0.7"
  depends_on :macos

  on_macos do
    on_arm do
      url "https://github.com/wukongnotnull/vole/releases/download/v0.0.7/vole-0.0.7-aarch64-apple-darwin.tar.gz"
      sha256 "03c1d897c00e9bc50b5fc56b687d5dcb2ef61993e4cfd9399bd67167eb7b40a4"
    end
    on_intel do
      url "https://github.com/wukongnotnull/vole/releases/download/v0.0.7/vole-0.0.7-x86_64-apple-darwin.tar.gz"
      sha256 "ecb05f2704e4972112236e1afaad9e8e8e93b073c857a5fa4bcab80a7dc4f57e"
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

      Stable bottles are ad-hoc signed (not notarized). If Gatekeeper blocks:
        xattr -cr #{bin}/vole
    EOS
  end

  test do
    assert_match "vole", shell_output("#{bin}/vole --help")
    assert_predicate share/"vole/rules", :directory?
  end
end
