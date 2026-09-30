class AgentOoo < Formula
  desc "Spa retreats for your AI. Reset, refresh, relax."
  homepage "https://github.com/mrtibbets/agent-ooo"
  version "0.1.4"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.4/agent-ooo-macos-aarch64.tar.gz"
      sha256 "3055bb18b4a29a7d7998310ae32b52e462920424b550668c5cb3e7ab61698de4"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.4/agent-ooo-linux-x86_64.tar.gz"
      sha256 "b8eab614484b47b9157b94f9020a2fbae61d054c0b6c94c4f9e7dbd70d4260f0"
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
