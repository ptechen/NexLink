pub static POSTGRES_POOL: std::sync::LazyLock<sqlx::postgres::PgPool> =
    std::sync::LazyLock::new(|| {
        let database_url = std::env::var("NEXLINK_POSTGRES_DSN")
            .unwrap_or_else(|_| "postgres://postgres:1234@127.0.0.1:5432/postgres".to_string());
        sqlx::postgres::PgPool::connect_lazy(&database_url).expect("connect postgres error")
    });
pub type Result<T> = std::result::Result<T, sqlx::Error>;
pub mod peer_user;
