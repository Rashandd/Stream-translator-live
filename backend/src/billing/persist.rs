//! Stripe webhook event'lerinin DB'ye kalıcılığı (Faz 1.11 follow-up).
//!
//! İki katman:
//! - [`action_for_event`] — saf eşleme: doğrulanmış event → yapılacak iş.
//!   DB'siz unit-testlenir.
//! - [`apply`] — aksiyonu `users`/`subscriptions` tablolarına uygular
//!   (runtime `sqlx::query`; compile-time macro değil, CI'da DATABASE_URL
//!   gerekmez). Eşleşmeyen kullanıcı/plan "soft-miss"tir: warn loglanır,
//!   webhook yine 200 döner (kullanıcı sistemi 1.4'te tamamlanınca doğal
//!   olarak eşleşir).
//!
//! Plan çözümü config üzerinden: `billing.plan.<plan>.stripe_price_id`
//! anahtarlarının tersine taranmasıyla price→plan adı bulunur (admin
//! panelden girilir, koda gömülü değildir).

use sqlx::PgPool;
use uuid::Uuid;

use crate::config::ServiceConfig;
use crate::logging::{LogLevel, Logger};

use super::stripe::StripeEvent;

/// Bir webhook event'inin DB karşılığı.
#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionAction {
    /// Checkout tamamlandı: `client_reference_id` (StreamDubber user UUID)
    /// ile Stripe müşterisi eşlenir.
    LinkCustomer {
        user_ref: String,
        customer_id: String,
    },
    /// Abonelik oluştu/güncellendi: `subscriptions` upsert edilir.
    Upsert {
        subscription_id: String,
        customer_id: String,
        status: String,
        period_start: i64,
        period_end: i64,
        price_id: Option<String>,
    },
    /// Abonelik silindi: durum `canceled` yapılır.
    Cancel { subscription_id: String },
    /// İlgilenmediğimiz event türü.
    Ignore,
}

/// Doğrulanmış event'i aksiyona çevirir (saf, DB'siz).
pub fn action_for_event(event: &StripeEvent) -> SubscriptionAction {
    let object = &event.data.object;
    let text = |key: &str| object[key].as_str().map(str::to_string);

    match event.event_type.as_str() {
        "checkout.session.completed" => {
            let (Some(user_ref), Some(customer_id)) = (text("client_reference_id"), text("customer"))
            else {
                return SubscriptionAction::Ignore;
            };
            SubscriptionAction::LinkCustomer { user_ref, customer_id }
        }
        "customer.subscription.created" | "customer.subscription.updated" => {
            let (Some(subscription_id), Some(customer_id), Some(status)) =
                (text("id"), text("customer"), text("status"))
            else {
                return SubscriptionAction::Ignore;
            };
            let (Some(period_start), Some(period_end)) = (
                object["current_period_start"].as_i64(),
                object["current_period_end"].as_i64(),
            ) else {
                return SubscriptionAction::Ignore;
            };
            let price_id = object["items"]["data"][0]["price"]["id"]
                .as_str()
                .map(str::to_string);
            SubscriptionAction::Upsert {
                subscription_id,
                customer_id,
                status,
                period_start,
                period_end,
                price_id,
            }
        }
        "customer.subscription.deleted" => match text("id") {
            Some(subscription_id) => SubscriptionAction::Cancel { subscription_id },
            None => SubscriptionAction::Ignore,
        },
        _ => SubscriptionAction::Ignore,
    }
}

/// `billing.plan.<plan>.stripe_price_id` anahtarlarını tersine tarayarak
/// price → plan adı çözer.
pub fn plan_for_price(cfg: &ServiceConfig, price_id: &str) -> Option<String> {
    if price_id.is_empty() {
        return None;
    }
    cfg.snapshot().into_iter().find_map(|(key, value)| {
        let plan = key
            .strip_prefix("billing.plan.")?
            .strip_suffix(".stripe_price_id")?;
        (value == price_id).then(|| plan.to_string())
    })
}

/// Aksiyonu DB'ye uygular. Soft-miss'ler (eşleşmeyen kullanıcı/plan) warn
/// loglanır ve `Ok(false)` döner; gerçek DB hataları `Err` (çağıran loglar,
/// webhook yanıtı yine 200'dür — Stripe'ın sonsuz retry gürültüsü yerine
/// log/alarm tercih edildi).
pub async fn apply(
    pool: &PgPool,
    action: &SubscriptionAction,
    cfg: &ServiceConfig,
    logger: &Logger,
) -> Result<bool, sqlx::Error> {
    match action {
        SubscriptionAction::Ignore => Ok(false),

        SubscriptionAction::LinkCustomer { user_ref, customer_id } => {
            let Ok(user_id) = Uuid::parse_str(user_ref) else {
                logger.log(
                    LogLevel::Warn,
                    "billing",
                    &format!("checkout completed but client_reference_id is not a UUID: {user_ref}"),
                    None,
                    None,
                );
                return Ok(false);
            };
            let result = sqlx::query(
                "UPDATE users SET stripe_customer_id = $2, updated_at = NOW() WHERE id = $1",
            )
            .bind(user_id)
            .bind(customer_id)
            .execute(pool)
            .await?;
            if result.rows_affected() == 0 {
                logger.log(
                    LogLevel::Warn,
                    "billing",
                    &format!("no user row for checkout client_reference_id {user_ref} (user system 1.4)"),
                    Some(user_ref),
                    None,
                );
                return Ok(false);
            }
            logger.log(
                LogLevel::Info,
                "billing",
                "stripe customer linked to user",
                Some(user_ref),
                None,
            );
            Ok(true)
        }

        SubscriptionAction::Upsert {
            subscription_id,
            customer_id,
            status,
            period_start,
            period_end,
            price_id,
        } => {
            // Kullanıcı: checkout.session.completed'ın yazdığı eşleme üzerinden.
            let user_id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM users WHERE stripe_customer_id = $1")
                    .bind(customer_id)
                    .fetch_optional(pool)
                    .await?;
            let Some(user_id) = user_id else {
                logger.log(
                    LogLevel::Warn,
                    "billing",
                    &format!("subscription {subscription_id}: no user with stripe customer id"),
                    None,
                    None,
                );
                return Ok(false);
            };

            // Plan: price → plan adı (config) → plans.id.
            let plan_name = price_id
                .as_deref()
                .and_then(|p| plan_for_price(cfg, p));
            let Some(plan_name) = plan_name else {
                logger.log(
                    LogLevel::Warn,
                    "billing",
                    &format!(
                        "subscription {subscription_id}: price not mapped to a plan (set billing.plan.<plan>.stripe_price_id)"
                    ),
                    None,
                    None,
                );
                return Ok(false);
            };
            let plan_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM plans WHERE name = $1")
                .bind(&plan_name)
                .fetch_optional(pool)
                .await?;
            let Some(plan_id) = plan_id else {
                logger.log(
                    LogLevel::Warn,
                    "billing",
                    &format!("subscription {subscription_id}: plan '{plan_name}' not in plans table"),
                    None,
                    None,
                );
                return Ok(false);
            };

            sqlx::query(
                "INSERT INTO subscriptions \
                     (user_id, plan_id, stripe_subscription_id, status, \
                      current_period_start, current_period_end) \
                 VALUES ($1, $2, $3, $4, to_timestamp($5), to_timestamp($6)) \
                 ON CONFLICT (stripe_subscription_id) DO UPDATE SET \
                     plan_id = EXCLUDED.plan_id, \
                     status = EXCLUDED.status, \
                     current_period_start = EXCLUDED.current_period_start, \
                     current_period_end = EXCLUDED.current_period_end, \
                     updated_at = NOW()",
            )
            .bind(user_id)
            .bind(plan_id)
            .bind(subscription_id)
            .bind(status)
            .bind(period_start)
            .bind(period_end)
            .execute(pool)
            .await?;
            logger.log(
                LogLevel::Info,
                "billing",
                &format!("subscription {subscription_id} upserted ({plan_name}, {status})"),
                None,
                None,
            );
            Ok(true)
        }

        SubscriptionAction::Cancel { subscription_id } => {
            let result = sqlx::query(
                "UPDATE subscriptions SET status = 'canceled', updated_at = NOW() \
                 WHERE stripe_subscription_id = $1",
            )
            .bind(subscription_id)
            .execute(pool)
            .await?;
            logger.log(
                LogLevel::Info,
                "billing",
                &format!("subscription {subscription_id} canceled"),
                None,
                None,
            );
            Ok(result.rows_affected() > 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::stripe::parse_event;

    fn event(json: &str) -> StripeEvent {
        parse_event(json.as_bytes()).expect("parse event")
    }

    #[test]
    fn checkout_completed_links_customer() {
        let e = event(
            r#"{"id":"evt_1","type":"checkout.session.completed",
                "data":{"object":{"id":"cs_1","customer":"cus_9",
                "client_reference_id":"11111111-1111-1111-1111-111111111111"}}}"#,
        );
        assert_eq!(
            action_for_event(&e),
            SubscriptionAction::LinkCustomer {
                user_ref: "11111111-1111-1111-1111-111111111111".into(),
                customer_id: "cus_9".into(),
            }
        );
    }

    #[test]
    fn subscription_updated_maps_to_upsert_with_price() {
        let e = event(
            r#"{"id":"evt_2","type":"customer.subscription.updated",
                "data":{"object":{"id":"sub_1","customer":"cus_9","status":"active",
                "current_period_start":1750000000,"current_period_end":1752600000,
                "items":{"data":[{"price":{"id":"price_pro"}}]}}}}"#,
        );
        assert_eq!(
            action_for_event(&e),
            SubscriptionAction::Upsert {
                subscription_id: "sub_1".into(),
                customer_id: "cus_9".into(),
                status: "active".into(),
                period_start: 1_750_000_000,
                period_end: 1_752_600_000,
                price_id: Some("price_pro".into()),
            }
        );
    }

    #[test]
    fn subscription_deleted_maps_to_cancel() {
        let e = event(
            r#"{"id":"evt_3","type":"customer.subscription.deleted",
                "data":{"object":{"id":"sub_1"}}}"#,
        );
        assert_eq!(
            action_for_event(&e),
            SubscriptionAction::Cancel { subscription_id: "sub_1".into() }
        );
    }

    #[test]
    fn missing_fields_and_unknown_types_are_ignored() {
        // customer alanı yok → Ignore.
        let e = event(
            r#"{"id":"evt_4","type":"checkout.session.completed","data":{"object":{"id":"cs_1"}}}"#,
        );
        assert_eq!(action_for_event(&e), SubscriptionAction::Ignore);

        // Dönem alanları yok → Ignore.
        let e = event(
            r#"{"id":"evt_5","type":"customer.subscription.updated",
                "data":{"object":{"id":"sub_1","customer":"cus_9","status":"active"}}}"#,
        );
        assert_eq!(action_for_event(&e), SubscriptionAction::Ignore);

        let e = event(r#"{"id":"evt_6","type":"invoice.paid","data":{"object":{}}}"#);
        assert_eq!(action_for_event(&e), SubscriptionAction::Ignore);
    }

    #[test]
    fn plan_for_price_reads_config_reverse_map() {
        let cfg = ServiceConfig::with_defaults();
        cfg.set("billing.plan.pro.stripe_price_id", "price_pro");
        assert_eq!(plan_for_price(&cfg, "price_pro").as_deref(), Some("pro"));
        assert_eq!(plan_for_price(&cfg, "price_ghost"), None);
        // Boş price hiçbir plana eşlenmez (seed'de boş string'ler var).
        assert_eq!(plan_for_price(&cfg, ""), None);
    }

    /// Gerçek Postgres ile uçtan uca persist. Çalıştırmak için:
    /// `TEST_DATABASE_URL=postgres://... cargo test -- --ignored stripe_persist`
    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with a migrated database"]
    async fn stripe_persist_roundtrip_against_real_db() {
        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        let pool = crate::db::connect(&url).await.expect("connect");
        crate::db::run_migrations(&pool).await.expect("migrations");

        let cfg = ServiceConfig::with_defaults();
        cfg.set("billing.plan.pro.stripe_price_id", "price_pro_test");
        let logger = Logger::in_memory(std::sync::Arc::new(ServiceConfig::with_defaults()));

        // Test kullanıcısı (plans seed'i migration'da yoksa pro planı ekle).
        let plan_id: Uuid = sqlx::query_scalar(
            "INSERT INTO plans (name, price_cents, monthly_hours, max_languages, max_platforms) \
             VALUES ('pro', 1500, 40, 10, -1) \
             ON CONFLICT (name) DO UPDATE SET updated_at = NOW() RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .expect("plan");
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, password_hash, plan_id) \
             VALUES ('persist-test@sd.io', 'x', $1) \
             ON CONFLICT (email) DO UPDATE SET updated_at = NOW() RETURNING id",
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .expect("user");

        // 1) checkout.session.completed → müşteri eşlenir.
        let link = SubscriptionAction::LinkCustomer {
            user_ref: user_id.to_string(),
            customer_id: "cus_persist_test".into(),
        };
        assert!(apply(&pool, &link, &cfg, &logger).await.expect("link"));

        // 2) subscription.updated → upsert.
        let upsert = SubscriptionAction::Upsert {
            subscription_id: "sub_persist_test".into(),
            customer_id: "cus_persist_test".into(),
            status: "active".into(),
            period_start: 1_750_000_000,
            period_end: 1_752_600_000,
            price_id: Some("price_pro_test".into()),
        };
        assert!(apply(&pool, &upsert, &cfg, &logger).await.expect("upsert"));

        // 3) deleted → canceled.
        let cancel = SubscriptionAction::Cancel { subscription_id: "sub_persist_test".into() };
        assert!(apply(&pool, &cancel, &cfg, &logger).await.expect("cancel"));

        let status: String = sqlx::query_scalar(
            "SELECT status FROM subscriptions WHERE stripe_subscription_id = 'sub_persist_test'",
        )
        .fetch_one(&pool)
        .await
        .expect("status");
        assert_eq!(status, "canceled");
    }
}
