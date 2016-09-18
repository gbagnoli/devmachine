
mount '/srv' do
  device '/dev/sda2'
  fstype 'btrfs'
  action [:mount, :enable]
end
