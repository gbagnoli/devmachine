# module with various helpers for LXD
module LXD
  # module Container offers helpers to get info about existing container
  module Container
    include Chef::Mixin::ShellOut
    def get_ipv4_address(container_name, opts = {})
      # rubocop:disable LineLength
      device = opts[:device] || 'eth0'
      command = %[lxc ls --format json | jq -r '.[] | select(.name == "#{container_name}") | .state.network.#{device}.addresses[] | select(.family == "inet") | .address']
      shell_out(command).stdout.strip
      # rubocop:enable LineLength
    end

    def get_ipv6_address(container_name, opts = {})
      # rubocop:disable LineLength
      device = opts[:device] || 'eth0'
      command = %[lxc ls --format json | jq -r '.[] | select(.name == "#{container_name}") | .state.network.#{device}.addresses[] | select(.family == "inet6") | select(.address | startswith("fe80") | not) | .address']
      shell_out(command).stdout.strip
      # rubocop:enable LineLength
    end
  end
end
