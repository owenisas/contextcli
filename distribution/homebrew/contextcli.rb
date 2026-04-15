class Contextcli < Formula
  desc "Universal CLI profile launcher — run any dev CLI under a named auth profile"
  homepage "https://github.com/your-org/contextcli"
  url "https://github.com/your-org/contextcli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--path", "crates/contextcli", "--root", prefix
  end

  test do
    assert_match "registered apps", shell_output("#{bin}/contextcli apps 2>&1")
  end
end
