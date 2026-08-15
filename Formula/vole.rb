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
  version "2.19.1"
  depends_on :macos

  on_macos do
    # sha256: pin after GitHub Release assets exist
    #   bash scripts/update-homebrew-formula.sh 2.19.1
    on_arm do
      url "https://github.com/wukongnotnull/vole/releases/download/v2.19.1/vole-2.19.1-aarch64-apple-darwin.tar.gz"
      sha256 "9d8f85b7cf0d22f7a3294e09c507c14ff52fff6ec1db0a5bb22f4bab7cb7a0d3"
    end
    on_intel do
      url "https://github.com/wukongnotnull/vole/releases/download/v2.19.1/vole-2.19.1-x86_64-apple-darwin.tar.gz"
      sha256 "c30bf70b59e1ae585371477450e63f6713fd546c4478c544174cb519f2c9756e"
    end
  end

  head do
    url "https://github.com/wukongnotnull/vole.git", branch: "main"
    depends_on "rust" => :build
  end

  def install
    if build.stable?
      # Homebrew often extracts a single top-level archive dir into buildpath.
      root = if (buildpath/"bin/vole").exist?
        buildpath
      else
        prefix_dir = Dir["vole-#{version}-*"].first
        odie "unexpected tarball layout (expected vole-#{version}-<arch>)" if prefix_dir.nil?
        buildpath/prefix_dir
      end
      bin.install root/"bin/vole"
      (share/"vole/rules").install Dir[root/"share/vole/rules/*.toml"]
    else
      system "cargo", "install", *std_cargo_args(path: "crates/vole-cli")
      pkgshare.install "data/rules"
    end
  end

  def caveats
    <<~EOS
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
