package 'lxd'
package 'lxd-client'

node['lxd']['storage']['pools'].each do |name, conf|
  if conf['driver'].nil? || conf['driver'].empty?
    Chef::Application.fatal!("Empty driver config for lxc storage pool #{name}")
  end
  attrs = conf['attrs'].to_h.keys.reduce('') do |memo, key|
    "#{memo} #{key}=#{conf['attrs'][key]}"
  end
  create = "lxc storage create #{name} #{conf['driver']} #{attrs}"
  info = "lxc storage info #{name} &>/dev/null"
  command = "#{info} || #{create}"

  execute "create storage #{name}" do
    command command
  end
end
