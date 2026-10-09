# Server email delivery follow-up

Status: requested; provider selection and implementation pending. Finish UniFi
restore acceptance first.

## Scope

- Allow server applications, including UniFi password recovery, to send email
  accepted by Gmail through an authenticated external delivery provider.
- Inspect existing Chef configuration and the operator’s existing provider key.
  The clear recipe `site-cookbooks/calculon/recipes/wanderer.rb` passes SMTP
  username/password/host/port/sender settings from encrypted configuration;
  the provider is not identified in the clear source. Do not publish the key.
- Evaluate a shared local SMTP relay backed by that provider versus direct
  application SMTP configuration. Keep the relay private and authenticated or
  otherwise restricted to declared local applications; prevent open relaying.
- Configure a verified sender domain and the provider’s required SPF/DKIM/DMARC
  records. Determine provider limits and suitability before choosing a service.
- Store credentials in KeePass, use generic configuration templates and host
  capability declarations, and update the root secrets checklist when actual
  requirements are introduced. No new vault keys are required yet.

## Acceptance

Send a test message from a configured application to an operator-controlled
Gmail inbox; inspect authentication results and delivery. Verify UniFi recovery
email works and unauthorized relay attempts are rejected. Record evidence
without recipient addresses, real domains or credentials.
