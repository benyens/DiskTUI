# Fórmula de Homebrew. Compila desde el código (tarda un par de minutos).
#
#   brew tap benyens/disktui https://github.com/benyens/DiskTUI
#   brew install --HEAD disktui
class Disktui < Formula
  desc "Visualizador de audio en la terminal: tu música crece como un jardín"
  homepage "https://github.com/benyens/DiskTUI"
  head "https://github.com/benyens/DiskTUI.git", branch: "main"

  depends_on "rust" => :build
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "DiskTUI", shell_output("#{bin}/disktui --help")
  end
end
