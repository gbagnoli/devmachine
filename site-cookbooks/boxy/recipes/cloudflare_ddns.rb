node.override["cloudflare_ddns"]["config"]["cloudflare"] = [{
  "authentication" => {
    "api_token" => node["cloudflare"]["dns_api_token"]
  },
  "zone_id" => node["boxy"]["ddns"]["zone_id"],
  "subdomains" => node["boxy"]["ddns"]["subdomains"],
}]
node.override["cloudflare_ddns"]["config"]["aaaa"] = false
node.override["cloudflare_ddns"]["config"]["purgeUnknownRecords"] = false

include_recipe "cloudflare_ddns"
