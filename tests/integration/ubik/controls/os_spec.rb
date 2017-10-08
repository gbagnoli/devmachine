include_controls 'linux-baseline' do
  skip_control 'package-08' # auditd
  skip_control 'os-10' # filesystems blacklist
  skip_control 'sysctl-18' # ipv6
end
