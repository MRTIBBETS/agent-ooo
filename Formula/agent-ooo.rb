class AgentOoo < Formula
  desc "Spa retreats for your AI. Reset, refresh, relax."
  homepage "https://github.com/mrtibbets/agent-ooo"
  version "0.1.6"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.6/agent-ooo-aarch64-apple-darwin.tar.gz"
      sha256 "941bf3e79ef0f9683df19d166c086a1f2cd72378bf9a65f137c328be0ecc26c1"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.6/agent-ooo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "09b98293e664431db89620facbf9013754f40e358e58f33a0e2fb12c7e9f4271"
    end
  end

  def install
    bin.install "agent-ooo"
    bin.install "ooo"
    bin.install "agent-ooo-mcp"
  end

  test do
    system "#{bin}/agent-ooo", "--help"
    system "#{bin}/ooo", "--help"
    system "#{bin}/agent-ooo-mcp", "--help"
  end
end
