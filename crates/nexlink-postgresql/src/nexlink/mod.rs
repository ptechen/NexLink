
pub static POSTGRES_POOL:std::sync::LazyLock<sqlx::postgres::PgPool> = std::sync::LazyLock::new(|| {
    sqlx::postgres::PgPool::connect_lazy("postgres://postgres:1234@127.0.0.1:5432/postgres").expect("connect postgres error")
});
pub type Result<T> = std::result::Result<T, sqlx::Error>;
pub mod peer_user;
