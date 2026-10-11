class Semaprax < Formula
  desc "Systems language: meaning in, verified machine code out"
  homepage "https://github.com/wavect/semaprax"
  license "Apache-2.0"

  livecheck do
    url :stable
    strategy :github_latest
  end

  depends_on :macos

  on_macos do
    on_arm do
      url "https://github.com/wavect/semaprax/releases/download/v0.9.0/semaprax-v0.9.0-aarch64-apple-darwin.tar.gz"
      sha256 "2cc1a432703413324549b9a54bef58d6134374ae9f721e542cde53877880de24"
    end
    on_intel do
      url "https://github.com/wavect/semaprax/releases/download/v0.9.0/semaprax-v0.9.0-x86_64-apple-darwin.tar.gz"
      sha256 "7af4ddf026c806e1c588bfbab7e1ec5df55db31f398baeaef3d7a1e12b363547"
    end
  end

  def install
    bin.install "semaprax", "semapraxd"
    pkgshare.install "release-manifest.json"
    prefix.install "LICENSE"
  end

  test do
    # Both executables ship in the same archive, so they are the same release.
    assert_path_exists bin/"semapraxd"
    assert_predicate bin/"semapraxd", :executable?
    assert_match version.to_s, shell_output("#{bin}/semaprax --version")
    assert_match version.to_s, (pkgshare/"release-manifest.json").read

    system bin/"semaprax", "new", "first-semaprax"
    system bin/"semaprax", "check", "first-semaprax/semaprax.toml"
    system bin/"semaprax", "test", "first-semaprax/semaprax.toml"
    assert_equal "42", shell_output("#{bin}/semaprax run first-semaprax/semaprax.toml").strip
  end
end
