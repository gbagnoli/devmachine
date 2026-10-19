# Homer dashboard

Homer is a shared private UI on every declared fleet host. Skillet renders
`/etc/skillet/homer/assets/config.yml` from that host's selected Caddy UI sites,
excluding the dashboard itself. The container has no published port; Caddy
reaches it over an isolated bridge and serves the generated YAML after applying
the same tailnet identity policy as other private UIs. Homer keeps its packaged
static assets. No Homer credential or second service inventory is needed.

Edit [`src/template.yml`](src/template.yml) to change the shared layout or theme,
then rebuild and apply Skillet. Skillet replaces the generated service list and
host subtitle during rendering. Runtime edits to the generated file are not
retained on the next apply.

Pi-hole and Home Assistant use Homer's smart-card types. The Pi-hole card links
to `/admin/` while its separate API endpoint stays at the origin root for v6
API calls. Pi-hole uses its canonical backend `Host` and `Origin` values (`pi.hole`);
Caddy allows only the declared Homer origins to read its API while retaining
the tailnet identity check. If the host has the existing
`skillet/environments/<environment>/hosts/<host>/homeassistant/api-token` entry,
Skillet adds it to the dashboard config for the Home Assistant card. The config
file is mode `0600` on the host and Caddy serves it only behind the tailnet
identity gate. Anyone authorized to view the dashboard can inspect that API
token in their browser, so keep the Homer allowlist limited to trusted users.

Service logos are bundled in [`assets/`](assets/). Syncthing and Mikochi use
their upstream logos, avoiding a runtime CDN dependency.

Open the dashboard at `<host>.<ui-domain>`, the existing machine A/AAAA name.
The zone-level `ui.<host>.<zone>` alias and canonical
`homer.<host>.<ui-domain>` name also work. Actual domains come from the selected
environment's Cloudflare zone configuration.
