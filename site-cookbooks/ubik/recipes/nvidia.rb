apt_repository "graphics_drivers" do
  uri "ppa:graphics-drivers/ppa"
end

package "nvidia" do
  package_name %w(nvidia-graphics-drivers-440 nvidia-settings)
end
