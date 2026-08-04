if platform?("rocky")
  execute "enable rocky crb" do
    command "dnf config-manager --set-enabled crb"
    not_if "grep -E 'crb.$' /etc/yum.repos.d/rocky.repo -A 5 | grep enabled=1 -q"
  end
end

yum_epel "testing" do
  repositories %w[epel epel-testing]
  enabled_repositories %w[epel epel-testing]
end

yum_elrepo 'default'
yum_elrepo_extras 'default'
yum_elrepo_kernel 'default'

