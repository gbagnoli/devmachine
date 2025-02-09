package "et_deps" do
  package_name %w(apt-transport-https software-properties-common)
end

apt_repository "git" do
  uri "ppa:jgmath2000/et"
end

package "et"

service "et" do
  action %i{enable start}
end
