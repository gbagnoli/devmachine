module Flexo
  # get the tdarr download URL
  module Tdarr
    def self.included(_base)
      require "json"
    end

    def tdarr_urls
    req = Chef::HTTP.new("https://f000.backblazeb2.com/file/tdarrs/versions.json")
    headers = { "Accept" => "application/json" }
    res = JSON.parse(req.request('get', req.url, headers))
    version = res.keys.max
    [version, res[version]["linux_x64"]["Tdarr_Server"], res[version]["linux_x64"]["Tdarr_Node"]]
    end
  end
end
