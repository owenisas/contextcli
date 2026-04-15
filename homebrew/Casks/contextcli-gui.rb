cask "contextcli-gui" do
  version "0.1.0"
  sha256 :no_check

  on_arm do
    url "https://github.com/owenisas/contextcli/releases/download/v#{version}/ContextCLI-v#{version}-aarch64-apple-darwin.zip"
  end

  on_intel do
    url "https://github.com/owenisas/contextcli/releases/download/v#{version}/ContextCLI-v#{version}-x86_64-apple-darwin.zip"
  end

  name "ContextCLI"
  desc "Desktop app for managing CLI auth profiles"
  homepage "https://github.com/owenisas/contextcli"

  app "ContextCLI.app"

  zap trash: "~/.contextcli"
end
