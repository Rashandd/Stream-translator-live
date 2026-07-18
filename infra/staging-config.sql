-- These are documented service_config keys. They are applied only after the
-- backend has completed its embedded SQLx migrations.
INSERT INTO service_config (key, value, description) VALUES
  ('providers.mock_fallback', 'false', 'Staging production-mode safety setting'),
  ('integration.redirect_base', 'https://dev.api.strovak.com/api/integrations', 'Staging integration OAuth callback base'),
  ('integration.frontend_base', 'https://dev.strovak.com/en', 'Staging locale-aware frontend return base'),
  ('auth.oauth_redirect_base', 'https://dev.api.strovak.com/api/auth/oauth', 'Staging login OAuth callback base'),
  ('billing.success_url', 'https://dev.strovak.com/en/dashboard/billing?checkout=success', 'Staging billing success URL'),
  ('billing.cancel_url', 'https://dev.strovak.com/en/dashboard/billing?checkout=canceled', 'Staging billing cancellation URL'),
  ('cors.allowed_origins', 'https://dev.strovak.com', 'Staging dashboard CORS origin'),
  ('ext.twitch.enabled', 'true', 'Enable the isolated Twitch Extension staging EBS')
ON CONFLICT (key) DO UPDATE SET
  value = EXCLUDED.value,
  description = EXCLUDED.description,
  updated_at = NOW();
