apt_repository "kisak-mesa" do
  uri "ppa:kisak/kisak-mesa"
end

package "mesa drivers" do
  package_name %w[mesa-vulkan-drivers mesa-vulkan-drivers:i386 i965-va-driver]
  action :install
end
