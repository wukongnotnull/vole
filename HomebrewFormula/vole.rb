# Homebrew formula draft for vole (not yet publishable).
# Fill url/sha256 after the first GitHub Release.
class Vole < Formula
  desc "macOS cleanup and monitoring CLI (Mole-derived, GPL-3.0)"
  homepage "https://github.com/wukongnotnull/vole"
  # url "https://github.com/wukongnotnull/vole/releases/download/v0.1.0/vole-0.1.0-macos-arm64.tar.gz"
  # sha256 "REPLACE_ME"
  license "GPL-3.0-only"
  version "0.0.1"

  depends_on :macos

  def install
    odie "Release url/sha256 not set yet — see docs/findings/2026-07-phase5-signing.md"
  end

  test do
    assert_match "vole", shell_output("#{bin}/vole --help")
  end
end
