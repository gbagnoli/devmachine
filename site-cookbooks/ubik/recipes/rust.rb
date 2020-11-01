user = node["ubik"]["rust"]["user"]
version = node["ubik"]["rust"]["version"]
rustupsh = "/home/#{user}/.local/src/rustup.sh"
cargo_bin = "/home/#{user}/.cargo/bin"
rustup = "#{cargo_bin}/rustup"
env = {'HOME' => ::Dir.home(user), 'USER' => user}

remote_file rustupsh do
  action :create
  source "https://sh.rustup.rs"
  owner user
end

execute "install_rustup" do
  environment env
  command "sh #{rustupsh} -y"
  user user
  not_if { ::File.exist?(rustup) }
  notifies :run, "bash[install_rust]", :immediately
end

bash "install_rust" do
  action :nothing
  user user
  environment env
  code <<-EOH
  #{rustup} toolchain install #{version} --profile=default
  #{rustup} default #{version}
  #{rustup} component add rls
  #{rustup} component add rustfmt
  EOH
end

bash "update rustup" do
  user user
  environment env
  code <<-EOH
   #{rustup} self update
   #{rustup} update
  EOH
end

completions = "/home/#{user}/.local/share/bash-completion/completions"
directory completions do
  action :create
  recursive true
  mode '0755'
  owner user
end

bash "install rustup completions" do
  user user
  environment env
  code <<-EOH
  #{rustup} completions bash > #{completions}/rustup
  EOH
  not_if { ::File.exist?("#{completions}/rustup") }
end

bash "install cargo completions" do
  user user
  environment env
  code <<-EOH
  #{rustup} completions bash cargo > #{completions}/cargo
  EOH
  not_if { ::File.exist?("#{completions}/cargo") }
end
