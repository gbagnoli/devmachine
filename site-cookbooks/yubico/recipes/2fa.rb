include_recipe 'yubico::ppa'

package 'libpam-yubico'

id = node['yubico']['pam']['id']
key = node['yubico']['pam']['key']
mappings = node['yubico']['pam']['users']

raise Exception, 'Missing id or key for yubico' if id.nil? || key.nil?

optional = ''
optional = 'nullok' if node['yubico']['pam']['optional']
authf = '/etc/yubikey_mappings'

file '/etc/pam.d/sshd' do
  content "auth sufficient pam_yubico.so id=#{id} key=#{key} debug #{optional} authfile=#{authf} mode=client"
  mode '0444'
end

template '/etc/yubikey_mappings' do
  source 'yubikey_mappings.erb'
  mode '0440'
  variables(
    mappings: mappings
  )
end

node.override['ssh-hardening']['ssh']['server']['challenge_response_authentication'] = true
node.override['ssh-hardening']['ssh']['server']['extras']['AuthenticationMethods'] = \
  'publickey,keyboard-interactive:pam'
