# frozen_string_literal: true

package "git"
package "perl"
package "make"

if platform?("debian") && node["platform_version"].to_i < 9
  package "btrfs-tools"
else
  package "btrfs-progs"
end

git node["btrbk"]["src_dir"] do
  repository node["btrbk"]["repository"]
  revision node["btrbk"]["revision"]
  action :sync
  notifies :run, "execute[install btrbk]", :immediately
end

execute "install btrbk" do
  action :nothing
  command "make install"
  cwd node["btrbk"]["src_dir"]
end
