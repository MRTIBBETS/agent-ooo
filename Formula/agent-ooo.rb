class AgentOoo < Formula
  desc "Spa retreats for your AI. Reset, refresh, relax."
  homepage "https://github.com/mrtibbets/agent-ooo"
  version "0.1.5"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.5/agent-ooo-macos-aarch64.tar.gz"
      sha256 "ad3a115bceb42b4ca294c2220acfae3baf527bd62c37b610277a489bc2172d31"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.5/agent-ooo-linux-x86_64.tar.gz"
      sha256 "3a0f18858c04679cb0137f9c404b6a8534c665921c287729e2bc9f411f4a810e"
    end
  end

  def install
    bin.install "agent-ooo"
    bin.install "ooo"
  end

  test do
    system "#{bin}/agent-ooo", "--help"
    system "#{bin}/ooo", "--help"
  end
end
