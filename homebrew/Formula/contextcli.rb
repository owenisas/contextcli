class Contextcli < Formula
  desc "Universal CLI profile launcher — run any dev CLI under a named auth profile"
  homepage "https://github.com/owenisas/contextcli"
  license "MIT"

  on_arm do
    url "https://github.com/owenisas/contextcli/releases/download/v#{version}/contextcli-v#{version}-aarch64-apple-darwin.tar.gz"
  end

  on_intel do
    url "https://github.com/owenisas/contextcli/releases/download/v#{version}/contextcli-v#{version}-x86_64-apple-darwin.tar.gz"
  end

  def install
    bin.install "contextcli"
  end

  test do
    assert_match "registered apps", shell_output("#{bin}/contextcli apps 2>&1")
  end
end
