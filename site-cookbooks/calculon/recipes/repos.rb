if platform?("rocky")
  execute "enable rocky crb" do
    command "dnf config-manager --set-enabled crb"
    not_if "grep -E 'crb.$' /etc/yum.repos.d/rocky.repo -A 5 | grep enabled=1 -q"
  end
end

node.default["yum"]["epel-testing"]["enabled"] = true
node.default["yum"]["epel-testing"]["managed"] = true
include_recipe "yum-epel"

node.default["yum"]["elrepo"]["enabled"] = true
include_recipe "yum-elrepo"
