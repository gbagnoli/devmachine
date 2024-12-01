package 'wakeonlan'

node["server"]["wol"]["targets"].each do |server, mac|
  file "/usr/local/bin/wol_#{server}" do
    mode "0755"
    content <<~EOH
      #!/bin/sh
      set -eu
      wakeonlan #{mac}
    EOH
  end
end
