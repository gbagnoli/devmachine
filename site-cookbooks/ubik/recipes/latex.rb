# frozen_string_literal: true

return unless node["ubik"]["install_latex"]

package "texlive-full" do
  timeout 3600
end
