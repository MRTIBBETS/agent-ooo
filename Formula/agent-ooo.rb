class AgentOoo < Formula
  desc "Spa retreats for your AI. Reset, refresh, relax."
  homepage "https://github.com/mrtibbets/agent-ooo"
  version "0.2.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.2.0/agent-ooo-aarch64-apple-darwin.tar.gz"
      sha256 "ea89f7f6eb0cce37210bda6cb991d65ca1eb0b5bab67e6879dee3d169c288fe2"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.2.0/agent-ooo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "7cdfda3608398ff3d177133bb7f68d2c7252979f3a68575c5dc92537ff998d96"
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
