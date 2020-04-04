node.override["server"]["oauth2_proxy"]["instance_name"] = "media-tigc-eu"
node.override["server"]["oauth2_proxy"]["http_port"] = "4180"
node.override["server"]["oauth2_proxy"]["upstream_port"] = 4189
node.override["server"]["oauth2_proxy"]["auth_provider"] = "google"
node.override["server"]["oauth2_proxy"]["redirect-url"] = "https://media.tigc.eu/oauth2/callback"
#
# need to set this in the secrets file
node.default["server"]["oauth2_proxy"]["authenticated_emails"] = nil
node.default["server"]["oauth2_proxy"]["client-secret"] = nil
node.default["server"]["oauth2_proxy"]["client-id"] = nil
node.default["server"]["oauth2_proxy"]["cookie-secret"] = nil

include_recipe "server::oauth2_proxy"
