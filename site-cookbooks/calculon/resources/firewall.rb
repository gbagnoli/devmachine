resource_name :calculon_firewalld_port
provides :calculon_firewalld_port
unified_mode true

property :zone, [Symbol, String], default: "public"
property :port, [Array, String, Integer]
default_action :open

action :open do
  package "firewalld" if find_resource(:package, "firewalld").nil?
  if new_resource.port.nil?
    portspec = "--add-port=#{new_resource.name}"
    ports = [new_resource.name.to_s]
  elsif new_resource.port.is_a? Array
    portspec = new_resource.port.map { |port| "--add-port=#{port}"}.join(" ")
    ports = new_resource.port
  else
    portspec = "--add-port=#{new_resource.port}"
    ports = [port.to_s]
  end

  existing = shell_out("firewall-cmd --zone=#{new_resource.zone} --list-ports").stdout.strip.split

  bash "firewalld_open_ports_#{ports.join(",")}@#{new_resource.zone}" do
    code <<~EOH
      firewall-cmd --zone=#{new_resource.zone} #{portspec}
      firewall-cmd --permanent --zone=#{new_resource.zone} #{portspec}
    EOH
    not_if { (ports - existing).empty? }
  end
end
