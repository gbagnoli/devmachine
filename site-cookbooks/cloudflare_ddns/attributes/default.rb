default["cloudflare_ddns"]["directory"] = "/etc/cloudflare_ddns"
default["cloudflare_ddns"]["config"] = {
  "cloudflare" => [],
  "a" => true,
  "aaaa" => false,
  "purgeUnknownRecords" => false,
  "ttl" => 300
}
