# frozen_string_literal: true

group "syslog" if node["platform"] == "ubuntu"

include_recipe "os-hardening"
include_recipe "ssh-hardening::server"
include_recipe "ssh-hardening::client"
