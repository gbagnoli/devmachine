apt_repository 'tailscale' do
  uri 'https://pkgs.tailscale.com/stable/ubuntu'
  key 'https://pkgs.tailscale.com/stable/ubuntu/jammy.noarmor.gpg'
  components ['main']
end

package 'tailscale'
