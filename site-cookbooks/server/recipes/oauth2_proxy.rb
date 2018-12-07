node.override['oauth2_proxy']['install_url'] = 'https://github.com/bitly/oauth2_proxy/releases/download/v2.2/oauth2_proxy-2.2.0.linux-amd64.go1.8.1.tar.gz'
node.override['oauth2_proxy']['checksum'] = '1c16698ed0c85aa47aeb80e608f723835d9d1a8b98bd9ae36a514826b3acce56'
node.override['oauth2_proxy']['install_path'] = '/usr/local/oauth2_proxy'
include_recipe 'oauth2_proxy::install'
