class AgentOoo < Formula
  desc "Spa retreats for your AI. Reset, refresh, relax."
  homepage "https://github.com/mrtibbets/agent-ooo"
  version "0.1.7"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.7/agent-ooo-aarch64-apple-darwin.tar.gz"
      sha256 "70c18dc7051872fdb55d7c0add9d2f1d21ac8128e7390c1c936612ce036e58ce"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/mrtibbets/agent-ooo/releases/download/v0.1.7/agent-ooo-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "425525035acbbd1517d263b1273d09db2d27d2d42727f7e592f7adfa0d97aad1"
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
