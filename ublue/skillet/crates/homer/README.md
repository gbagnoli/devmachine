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

The dashboard has a zone-level `ui.<host>.<zone>` alias. Its canonical private
UI name also follows the ordinary `homer.<host>.<ui-domain>` convention. Actual
domains come from the selected environment's Cloudflare zone configuration.
