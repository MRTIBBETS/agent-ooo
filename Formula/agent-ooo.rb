class AgentOoo < Formula
  desc "Spa retreats for your AI. Reset, refresh, relax."
  homepage "https://github.com/mrtibbets/agent-ooo"
  version "0.3.0"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.3.0/agent-ooo-aarch64-apple-darwin.tar.gz"
      sha256 "31a29a98105a9b43558362fbf1d2d2c362d552d64b1282afdd7c29ee93c98d38"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.3.0/agent-ooo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "c769eb425a0d119ad62040f3a418c468cd72fce77d0d67c3927214700d381182"
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
