# Source for the Homebrew cask. The live tap (lcajigasm/homebrew-tap)
# is updated when a dmg that contains Contents/SharedSupport/bin/lux
# is published. v1.5.6 predates that binary, so this file is not the
# formula brew installs today.
cask "lux" do
  version "1.5.6"
  sha256 "cc93cb6837de469d09ce365dada97f1e3772cea50f199c39d6735b8d1ed18789"

  url "https://github.com/lcajigasm/lux/releases/download/v#{version}/lux-macos-universal.dmg"
  name "Lux"
  desc "Ultra-fast native code editor"
  homepage "https://github.com/lcajigasm/lux"

  depends_on :macos

  app "Lux.app"
  # Puts `lux` on PATH. The binary is the bundle CLI, which opens the
  # running editor or starts Lux.app.
  binary "#{appdir}/Lux.app/Contents/SharedSupport/bin/lux", target: "lux"

  zap trash: [
    "~/.config/lux",
    "~/Library/Saved Application State/dev.lux.editor.savedState",
  ]

  caveats <<~EOS
    Lux is ad-hoc signed and is not notarized, so Gatekeeper quarantines the app.
    Clear the quarantine attribute after install:

      xattr -dr com.apple.quarantine /Applications/Lux.app

    Or install without quarantine:

      brew install --cask --no-quarantine lcajigasm/tap/lux
  EOS
end
