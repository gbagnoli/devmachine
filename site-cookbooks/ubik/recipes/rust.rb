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

ruby_block "get rust-analyzer url" do
  block do
    Chef::Resource::RubyBlock.send(:include, Chef::Mixin::ShellOut) # rubocop:disable Lint/SendWithMixinArgument
    release = "https://api.github.com/repos/rust-analyzer/rust-analyzer/releases/latest"
    download_url = ".assets[].browser_download_url"
    command = "curl -sL #{release} | jq -r '#{download_url}' | grep x86_64-unknown-linux-gnu"
    out = shell_out(command)
    node.run_state["run_analyzer_url"] = out.stdout
  end
  action :run
end

remote_file "/usr/src/rust-analyzer.gz" do
  source lazy { node.run_state["run_analyzer_url"].chomp } # rubocop:disable Lint/AmbiguousBlockAssociation
  mode '644'
  notifies :run, "execute[unpack rust analyzer]", :immediately
end

execute "unpack rust analyzer" do
  command "gunzip -d /usr/src/rust-analyzer.gz -c > /usr/local/bin/rust-analyzer"
  action :nothing
end

file "/usr/local/bin/rust-analyzer" do
  mode '755'
end
