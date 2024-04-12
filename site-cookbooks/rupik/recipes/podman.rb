base="https://download.opensuse.org/repositories"
path="devel:/kubic:/libcontainers:/stable"
ubuntu="xUbuntu_#{node["platform_version"]}"
uri="#{base}/#{path}/#{ubuntu}/"
apt_repository "devel:kubic:libcontainers:stable" do
  uri uri
  distribution '/'
  components []
  key "#{uri}/Release.key"
end

package "podman"

service "podman.socket" do
  action %i(enable start)
end
