//! Mole-aligned clean whitelist catalog (from mole `get_all_cache_items`).

/// One predefined clean-whitelist row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanWhitelistItem {
    pub display_name: &'static str,
    /// Pattern with `$HOME` prefix, or `FINDER_METADATA` sentinel.
    pub pattern: &'static str,
}

/// Full clean whitelist inventory shown in `vole clean --whitelist`.
pub static CLEAN_WHITELIST_CATALOG: &[CleanWhitelistItem] = &[
    CleanWhitelistItem {
        display_name: "Apple Mail cache",
        pattern: "$HOME/Library/Caches/com.apple.mail/*",
    },
    CleanWhitelistItem {
        display_name: "Gradle build cache (Android Studio, Gradle projects)",
        pattern: "$HOME/.gradle/caches/build-cache-*/*",
    },
    CleanWhitelistItem {
        display_name: "Gradle daemon processes cache",
        pattern: "$HOME/.gradle/daemon/*",
    },
    CleanWhitelistItem {
        display_name: "Gradle worker cache",
        pattern: "$HOME/.gradle/workers/*",
    },
    CleanWhitelistItem {
        display_name: "Xcode DerivedData (build outputs, indexes)",
        pattern: "$HOME/Library/Developer/Xcode/DerivedData/*",
    },
    CleanWhitelistItem {
        display_name: "Xcode internal cache files",
        pattern: "$HOME/Library/Caches/com.apple.dt.Xcode/*",
    },
    CleanWhitelistItem {
        display_name: "Xcode iOS device support symbols",
        pattern:
            "$HOME/Library/Developer/Xcode/iOS DeviceSupport/*/Symbols/System/Library/Caches/*",
    },
    CleanWhitelistItem {
        display_name: "Maven local repository (Java dependencies)",
        pattern: "$HOME/.m2/repository/*",
    },
    CleanWhitelistItem {
        display_name: "JetBrains IDEs data (IntelliJ, PyCharm, WebStorm, GoLand)",
        pattern: "$HOME/Library/Application Support/JetBrains/*",
    },
    CleanWhitelistItem {
        display_name: "JetBrains IDEs cache",
        pattern: "$HOME/Library/Caches/JetBrains/*",
    },
    CleanWhitelistItem {
        display_name: "Android Studio cache and indexes",
        pattern: "$HOME/Library/Caches/Google/AndroidStudio*/*",
    },
    CleanWhitelistItem {
        display_name: "Android build cache",
        pattern: "$HOME/.android/build-cache/*",
    },
    CleanWhitelistItem {
        display_name: "VS Code runtime cache",
        pattern: "$HOME/Library/Application Support/Code/Cache/*",
    },
    CleanWhitelistItem {
        display_name: "VS Code extension and update cache",
        pattern: "$HOME/Library/Application Support/Code/CachedData/*",
    },
    CleanWhitelistItem {
        display_name: "VS Code system cache (Cursor, VSCodium)",
        pattern: "$HOME/Library/Caches/com.microsoft.VSCode/*",
    },
    CleanWhitelistItem {
        display_name: "Cursor editor cache",
        pattern: "$HOME/Library/Caches/com.todesktop.230313mzl4w4u92/*",
    },
    CleanWhitelistItem {
        display_name: "LM Studio app cache",
        pattern: "$HOME/Library/Caches/com.lmstudio.lmstudio/*",
    },
    CleanWhitelistItem {
        display_name: "Codex Desktop update staging",
        pattern: "$HOME/Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation",
    },
    CleanWhitelistItem {
        display_name: "Chrome on-device AI models",
        pattern: "$HOME/Library/Application Support/Google/Chrome/OptGuideOnDevice*/*",
    },
    CleanWhitelistItem {
        display_name: "Chrome optimization guide models",
        pattern: "$HOME/Library/Application Support/Google/Chrome/optimization_guide_model_store/*",
    },
    CleanWhitelistItem {
        display_name: "Bazel build cache",
        pattern: "$HOME/.cache/bazel/*",
    },
    CleanWhitelistItem {
        display_name: "Go build cache",
        pattern: "$HOME/Library/Caches/go-build/*",
    },
    CleanWhitelistItem {
        display_name: "Go module cache",
        pattern: "$HOME/go/pkg/mod/*",
    },
    CleanWhitelistItem {
        display_name: "Rust Cargo registry cache",
        pattern: "$HOME/.cargo/registry/cache/*",
    },
    CleanWhitelistItem {
        display_name: "Rust documentation cache",
        pattern: "$HOME/.rustup/toolchains/*/share/doc/*",
    },
    CleanWhitelistItem {
        display_name: "Rustup toolchain downloads",
        pattern: "$HOME/.rustup/downloads/*",
    },
    CleanWhitelistItem {
        display_name: "ccache compiler cache",
        pattern: "$HOME/.ccache/*",
    },
    CleanWhitelistItem {
        display_name: "sccache distributed compiler cache",
        pattern: "$HOME/.cache/sccache/*",
    },
    CleanWhitelistItem {
        display_name: "SBT Scala build cache",
        pattern: "$HOME/.sbt/*",
    },
    CleanWhitelistItem {
        display_name: "Ivy dependency cache",
        pattern: "$HOME/.ivy2/cache/*",
    },
    CleanWhitelistItem {
        display_name: "Turbo monorepo build cache",
        pattern: "$HOME/.turbo/*",
    },
    CleanWhitelistItem {
        display_name: "Next.js build cache",
        pattern: "$HOME/.next/*",
    },
    CleanWhitelistItem {
        display_name: "Vite build cache",
        pattern: "$HOME/.vite/*",
    },
    CleanWhitelistItem {
        display_name: "Parcel bundler cache",
        pattern: "$HOME/.parcel-cache/*",
    },
    CleanWhitelistItem {
        display_name: "pre-commit hooks cache",
        pattern: "$HOME/.cache/pre-commit/*",
    },
    CleanWhitelistItem {
        display_name: "Ruff Python linter cache",
        pattern: "$HOME/.cache/ruff/*",
    },
    CleanWhitelistItem {
        display_name: "MyPy type checker cache",
        pattern: "$HOME/.cache/mypy/*",
    },
    CleanWhitelistItem {
        display_name: "Pytest test cache",
        pattern: "$HOME/.pytest_cache/*",
    },
    CleanWhitelistItem {
        display_name: "Flutter SDK cache",
        pattern: "$HOME/.cache/flutter/*",
    },
    CleanWhitelistItem {
        display_name: "Swift Package Manager cache",
        pattern: "$HOME/.cache/swift-package-manager/*",
    },
    CleanWhitelistItem {
        display_name: "Zig compiler cache",
        pattern: "$HOME/.cache/zig/*",
    },
    CleanWhitelistItem {
        display_name: "Deno cache",
        pattern: "$HOME/Library/Caches/deno/*",
    },
    CleanWhitelistItem {
        display_name: "CocoaPods cache (iOS dependencies)",
        pattern: "$HOME/Library/Caches/CocoaPods/*",
    },
    CleanWhitelistItem {
        display_name: "npm package cache",
        pattern: "$HOME/.npm/_cacache/*",
    },
    CleanWhitelistItem {
        display_name: "pip Python package cache",
        pattern: "$HOME/.cache/pip/*",
    },
    CleanWhitelistItem {
        display_name: "uv Python package cache",
        pattern: "$HOME/.cache/uv/*",
    },
    CleanWhitelistItem {
        display_name: "R renv global cache (virtual environments)",
        pattern: "$HOME/Library/Caches/org.R-project.R/R/renv/*",
    },
    CleanWhitelistItem {
        display_name: "tealdeer tldr pages cache",
        pattern: "$HOME/Library/Caches/tealdeer/tldr-pages",
    },
    CleanWhitelistItem {
        display_name: "Homebrew downloaded packages",
        pattern: "$HOME/Library/Caches/Homebrew/*",
    },
    CleanWhitelistItem {
        display_name: "Yarn package manager cache",
        pattern: "$HOME/.cache/yarn/*",
    },
    CleanWhitelistItem {
        display_name: "pnpm package store",
        pattern: "$HOME/Library/pnpm/store/*",
    },
    CleanWhitelistItem {
        display_name: "Composer PHP dependencies cache (legacy)",
        pattern: "$HOME/.composer/cache/*",
    },
    CleanWhitelistItem {
        display_name: "Composer PHP dependencies cache",
        pattern: "$HOME/Library/Caches/composer/*",
    },
    CleanWhitelistItem {
        display_name: "RubyGems cache",
        pattern: "$HOME/.gem/cache/*",
    },
    CleanWhitelistItem {
        display_name: "Conda package metadata/tarball cache",
        pattern: "$HOME/.conda/pkgs",
    },
    CleanWhitelistItem {
        display_name: "Anaconda package metadata/tarball cache",
        pattern: "$HOME/anaconda3/pkgs",
    },
    CleanWhitelistItem {
        display_name: "PyTorch model cache",
        pattern: "$HOME/.cache/torch/*",
    },
    CleanWhitelistItem {
        display_name: "TensorFlow model and dataset cache",
        pattern: "$HOME/.cache/tensorflow/*",
    },
    CleanWhitelistItem {
        display_name: "HuggingFace models and datasets",
        pattern: "$HOME/.cache/huggingface/*",
    },
    CleanWhitelistItem {
        display_name: "Playwright browser binaries",
        pattern: "$HOME/Library/Caches/ms-playwright*",
    },
    CleanWhitelistItem {
        display_name: "Selenium WebDriver binaries",
        pattern: "$HOME/.cache/selenium/*",
    },
    CleanWhitelistItem {
        display_name: "Ollama local AI models",
        pattern: "$HOME/.ollama/models/*",
    },
    CleanWhitelistItem {
        display_name: "Weights & Biases ML experiments cache",
        pattern: "$HOME/.cache/wandb/*",
    },
    CleanWhitelistItem {
        display_name: "Safari web browser cache",
        pattern: "$HOME/Library/Caches/com.apple.Safari/*",
    },
    CleanWhitelistItem {
        display_name: "Chrome browser cache",
        pattern: "$HOME/Library/Caches/Google/Chrome/*",
    },
    CleanWhitelistItem {
        display_name: "Firefox browser cache",
        pattern: "$HOME/Library/Caches/Firefox/*",
    },
    CleanWhitelistItem {
        display_name: "Brave browser cache",
        pattern: "$HOME/Library/Caches/BraveSoftware/Brave-Browser/*",
    },
    CleanWhitelistItem {
        display_name: "Surge proxy cache",
        pattern: "$HOME/Library/Caches/com.nssurge.surge-mac/*",
    },
    CleanWhitelistItem {
        display_name: "Surge configuration and data",
        pattern: "$HOME/Library/Application Support/com.nssurge.surge-mac/*",
    },
    CleanWhitelistItem {
        display_name: "Docker BuildX cache",
        pattern: "$HOME/.docker/buildx/cache/*",
    },
    CleanWhitelistItem {
        display_name: "Podman container cache",
        pattern: "$HOME/.local/share/containers/cache/*",
    },
    CleanWhitelistItem {
        display_name: "Tart OCI/IPSW cache",
        pattern: "$HOME/.tart/cache",
    },
    CleanWhitelistItem {
        display_name: "Font cache",
        pattern: "$HOME/Library/Caches/com.apple.FontRegistry/*",
    },
    CleanWhitelistItem {
        display_name: "Spotlight metadata cache",
        pattern: "$HOME/Library/Caches/com.apple.spotlight/*",
    },
    CleanWhitelistItem {
        display_name: "CloudKit cache",
        pattern: "$HOME/Library/Caches/CloudKit/*",
    },
    CleanWhitelistItem {
        display_name: "Trash",
        pattern: "$HOME/.Trash",
    },
    CleanWhitelistItem {
        display_name: "iOS/iPadOS device firmware (.ipsw) from iTunes/Finder",
        pattern: "$HOME/Library/iTunes/*Software Updates/*.ipsw",
    },
    CleanWhitelistItem {
        display_name: "Apple Configurator 2 device firmware (.ipsw)",
        pattern: "$HOME/Library/Group Containers/*.group.com.apple.configurator/**/*.ipsw",
    },
    CleanWhitelistItem {
        display_name: "Finder metadata, .DS_Store",
        pattern: "FINDER_METADATA",
    },
];

/// Mole `DEFAULT_WHITELIST_PATTERNS` (manage-session seed when config missing).
pub static DEFAULT_CLEAN_WHITELIST_PATTERNS: &[&str] = &[
    "$HOME/Library/Caches/ms-playwright*",
    "$HOME/.cache/huggingface*",
    "$HOME/.m2/repository/*",
    "$HOME/.gradle/caches/*",
    "$HOME/.gradle/daemon/*",
    "$HOME/.ollama/models/*",
    "$HOME/Library/Caches/com.nssurge.surge-mac/*",
    "$HOME/Library/Application Support/com.nssurge.surge-mac/*",
    "$HOME/Library/Caches/org.R-project.R/R/renv/*",
    "$HOME/Library/Caches/pypoetry/virtualenvs*",
    "$HOME/Library/Caches/JetBrains*",
    "$HOME/Library/Caches/com.jetbrains.toolbox*",
    "$HOME/Library/Caches/tealdeer/tldr-pages",
    "$HOME/Library/Application Support/JetBrains*",
    "$HOME/Library/Caches/com.apple.finder",
    "$HOME/Library/Mobile Documents*",
    "$HOME/Library/Caches/com.apple.FontRegistry*",
    "$HOME/Library/Caches/com.apple.spotlight*",
    "$HOME/Library/Caches/com.apple.Spotlight*",
    "$HOME/Library/Caches/CloudKit*",
    "FINDER_METADATA",
];
