
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use super::Result;
use super::POSTGRES_POOL;

pub const TABLE_NAME: &str = "peer_user";

pub const FIELDS: &str = "id,peer_id,send,recv,total_limit,is_valid,updated_at,created_at";

/// Unique：[id]
/// Unique：[peer_id]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PeerUser {
	pub id: i64,
	pub peer_id: String,
	pub send: i64,
	pub recv: i64,
	pub total_limit: i64,
	pub is_valid: bool,
	#[serde(with = "time::serde::rfc3339::option", default)]
    pub updated_at: Option<time::OffsetDateTime>,
        
	#[serde(with = "time::serde::rfc3339::option", default)]
    pub created_at: Option<time::OffsetDateTime>,
        
}


impl PeerUser {
    pub async fn insert(&self) -> Result<u64> {
    	let sql = format!("INSERT INTO peer_user ({FIELDS}) VALUES($1,$2,$3,$4,$5,$6,$7,$8)");
    	let mut pool = POSTGRES_POOL.acquire().await?;
    	let data = sqlx::query(&sql)
             .bind(&self.id)
             .bind(&self.peer_id)
             .bind(&self.send)
             .bind(&self.recv)
             .bind(&self.total_limit)
             .bind(&self.is_valid)
             .bind(&self.updated_at)
             .bind(&self.created_at)
    	    .execute(&mut *pool)
            .await?.rows_affected();
        Ok(data)
    }
    

    
    pub async fn select_all() -> Result<Vec<Self>> {
        let sql = format!("SELECT {FIELDS} FROM {TABLE_NAME} ");
        let mut pool = POSTGRES_POOL.acquire().await?;
        let data = sqlx::query_as::<_, Self>(&sql).fetch_all(&mut *pool).await?;
        Ok(data)
    }
    


    
    pub async fn select_optional_by_id(id:i64,)->Result<Option<Self>>{
        let sql = format!("SELECT {FIELDS} FROM {TABLE_NAME} WHERE  id = $1 ");
        let mut pool = POSTGRES_POOL.acquire().await?;
        let data = sqlx::query_as::<_, Self>(&sql)
            .bind(id).fetch_optional(&mut *pool)
            .await?;
        Ok(data)
    }
    

    
    pub async fn select_optional_by_peer_id(peer_id:&str,)->Result<Option<Self>>{
        let sql = format!("SELECT {FIELDS} FROM {TABLE_NAME} WHERE  peer_id = $1 ");
        let mut pool = POSTGRES_POOL.acquire().await?;
        let data = sqlx::query_as::<_, Self>(&sql)
            .bind(peer_id).fetch_optional(&mut *pool)
            .await?;
        Ok(data)
    }
    

    
    pub async fn select_one_by_id(id:i64,)->Result<Self>{
        let sql = format!("SELECT {FIELDS} FROM {TABLE_NAME} WHERE  id = $1 ");
        let mut pool = POSTGRES_POOL.acquire().await?;
        let data = sqlx::query_as::<_, Self>(&sql)
            .bind(id).fetch_one(&mut *pool)
            .await?;
        Ok(data)
    }
    

    
    pub async fn select_one_by_peer_id(peer_id:&str,)->Result<Self>{
        let sql = format!("SELECT {FIELDS} FROM {TABLE_NAME} WHERE  peer_id = $1 ");
        let mut pool = POSTGRES_POOL.acquire().await?;
        let data = sqlx::query_as::<_, Self>(&sql)
            .bind(peer_id).fetch_one(&mut *pool)
            .await?;
        Ok(data)
    }
    
}

    
// ***************************************以下是自定义代码区域******************************************
/*
example: [
    {"skip_fields": ["updated_at", "created_at"], "filename": "table_name1"},
    {"contain_fields": ["updated_at", "created_at"], "filename": "table_name2"}
]
*/
// *************************************************************************************************

impl PeerUser {
    pub async fn update_insert(&self) -> Result<()> {
        match Self::select_optional_by_peer_id(&self.peer_id).await {
            Ok(_) => {
                self.insert().await?;
            }
            Err(_) => {}
        };
        Ok(())
    }

    /// INSERT ON CONFLICT DO NOTHING，返回数据库 id。
    /// 若 peer_id 已存在则 fallback 查询 id。
    pub async fn insert_if_not_exists(peer_id: &str) -> Result<i64> {
        let sql = "INSERT INTO peer_user (peer_id) VALUES ($1) ON CONFLICT (peer_id) DO NOTHING RETURNING id";
        let mut pool = POSTGRES_POOL.acquire().await?;
        let row = sqlx::query_scalar::<_, i64>(sql)
            .bind(peer_id)
            .fetch_optional(&mut *pool)
            .await?;
        match row {
            Some(id) => Ok(id),
            None => {
                let sql = "SELECT id FROM peer_user WHERE peer_id = $1";
                let id = sqlx::query_scalar::<_, i64>(sql)
                    .bind(peer_id)
                    .fetch_one(&mut *pool)
                    .await?;
                Ok(id)
            }
        }
    }
}